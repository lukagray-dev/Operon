// path_guard.rs — Path containment checking for directory-scoped tools.
//
// This module is responsible for one job: given an arbitrary file path from a
// tool call argument, determine which DirectoryPolicy (if any) covers it.
//
// All logic here is pure path manipulation. No I/O policy decisions, no
// PermissionMode lookups, no CallerRole awareness. Those live in resolver.rs.
//
// WHY A SEPARATE MODULE:
//   Path containment is subtle. Naive string prefix matching fails on:
//     - "/allowed" falsely covering "/allowedBUT"
//     - Symlinks that escape the allowed root (e.g. /allowed/link -> /etc)
//     - Windows drive letter case differences ("C:" vs "c:")
//     - Relative path segments ("..", ".", "//")
//   This module centralizes the correct implementation in one place.
//
// CANONICALIZATION INVARIANT:
//   Both the directory paths in DirectoryPolicy AND the tool argument path
//   passed to `find_directory()` must be canonicalized before comparison.
//   DirectoryPolicy paths are canonicalized by `PolicyConfig::validate()`.
//   The input path is canonicalized by `PathGuard::find_directory()` itself.
//
// SYMLINK ATTACK SURFACE:
//   `std::fs::canonicalize()` resolves symlinks. A symlink inside an allowed
//   directory that points outside it will resolve to the target — which is
//   outside the allowed root — and will fail the containment check.
//   This is the correct behavior: we block the traversal.

use std::path::{Path, PathBuf};
use crate::config::DirectoryPolicy;

// ─────────────────────────────────────────────────────────────────────────────
// PathGuard
// ─────────────────────────────────────────────────────────────────────────────

/// Checks whether a filesystem path falls within any allowed directory.
///
/// `PathGuard` is a stateless helper — it holds a reference to the slice of
/// `DirectoryPolicy` entries from `PolicyConfig` and provides a single method
/// `find_directory()` that resolves an arbitrary path against them.
///
/// # Construction
///
/// ```rust
/// use operon_policy::path_guard::PathGuard;
/// // PathGuard is typically created inside PolicyResolver — not directly.
/// ```
///
/// # Canonicalization contract
///
/// All `DirectoryPolicy.path` values must be canonical (call `PolicyConfig::validate()`
/// first). `find_directory()` will canonicalize the input path itself.
pub struct PathGuard<'a> {
    /// The list of allowed directories from `PolicyConfig`.
    /// Each path must be canonical (resolved via `PolicyConfig::validate()`).
    directories: &'a [DirectoryPolicy],
}

impl<'a> PathGuard<'a> {
    /// Creates a new `PathGuard` over the given directory policy slice.
    ///
    /// The caller must ensure all `DirectoryPolicy.path` values are canonical.
    /// Call `PolicyConfig::validate()` before constructing `PathGuard`.
    pub fn new(directories: &'a [DirectoryPolicy]) -> Self {
        Self { directories }
    }

    /// Finds the `DirectoryPolicy` that covers the given path, if any.
    ///
    /// Steps:
    ///   1. Canonicalize `path` using `std::fs::canonicalize()`.
    ///      - If canonicalization fails (path doesn't exist yet), fall back to
    ///        normalizing the path without I/O. We do NOT deny just because
    ///        the target file doesn't exist — the tool may be creating it.
    ///   2. Check whether the canonical path starts_with any directory's canonical path.
    ///      - `starts_with()` on `Path` is component-aware: "/allowed" does NOT
    ///        cover "/allowedBUT/file". It checks full path component boundaries.
    ///   3. Return the first matching `DirectoryPolicy`, or `None` if no match.
    ///
    /// # Arguments
    /// - `path`: The path argument extracted from the tool call (e.g. from `"path"` or `"cwd"`).
    ///
    /// # Returns
    /// - `Some(&DirectoryPolicy)` — the first matching policy (most-specific first if configured).
    /// - `None` — the path is outside all allowed directories → resolver must deny.
    pub fn find_directory(&self, path: &Path) -> Option<&'a DirectoryPolicy> {
        // Step 1: Canonicalize the input path.
        // For paths that already exist on disk, this resolves symlinks and ..
        // For paths that don't exist yet (new files being created), canonicalize
        // will fail — so we fall back to normalize_without_io().
        let canonical: PathBuf = match std::fs::canonicalize(path) {
            Ok(c) => c,
            Err(_) => {
                // File doesn't exist yet (e.g. model is creating a new file).
                // Normalize the path components without I/O to prevent .. traversal.
                // IMPORTANT: also strip the Windows extended-length prefix (\\?\)
                // if present, because std::fs::canonicalize() on Windows adds it,
                // meaning policy directory paths have it but our non-I/O normalized
                // paths won't — causing starts_with() to fail.
                normalize_without_io(path)
            }
        };

        // Step 2: Find the first DirectoryPolicy whose canonical root is a
        // path prefix of our canonical input. `Path::starts_with()` is
        // component-aware — it will NOT falsely match "/allowed" against "/allowedBUT".
        //
        // On Windows, std::fs::canonicalize() prefixes paths with \\?\ (extended-length
        // path prefix). Both the directory path (from PolicyConfig::validate()) and the
        // canonical input path will have this prefix, so starts_with() works correctly
        // for existing paths.
        //
        // For the normalize_without_io fallback (nonexistent files), we strip the \\?\
        // prefix from the directory path before comparing, so a nonexistent file path
        // (which won't have the \\?\ prefix) still matches against its parent directory.
        self.directories.iter().find(|dir| {
            // Try the direct starts_with first (covers all existing-file cases and Unix).
            if canonical.starts_with(&dir.path) {
                return true;
            }
            // Fallback for Windows: if the stored dir.path has the \\?\ prefix but our
            // normalize_without_io path does not, strip the prefix and retry.
            // This happens specifically when canonicalize() failed (nonexistent path).
            #[cfg(windows)]
            {
                let dir_str = dir.path.to_string_lossy();
                // \\?\ prefix is exactly 4 chars: \, \, ?, \
                if dir_str.starts_with(r"\\?\") {
                    let stripped = std::path::Path::new(&dir_str[4..]);
                    if canonical.starts_with(stripped) {
                        return true;
                    }
                }
            }
            false
        })
    }

    /// Returns true if the given path is covered by any allowed directory.
    ///
    /// Convenience wrapper around `find_directory()`.
    pub fn is_allowed(&self, path: &Path) -> bool {
        self.find_directory(path).is_some()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Private helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Normalizes a path without touching the filesystem.
///
/// Used as a fallback when `std::fs::canonicalize()` fails (e.g. the target
/// file doesn't exist yet). Resolves `..` and `.` components by walking the
/// path component list, but does NOT resolve symlinks.
///
/// This is safe for the policy check because:
///   - A real symlink traversal attack requires the symlink to exist on disk.
///   - If the path doesn't exist, there's no symlink to follow.
///   - Any `..` components are neutralized by the component walk below.
///
/// # Edge cases
///
/// - Paths with only `..` from the root (e.g. `/../../etc`) are clamped at the root.
/// - Relative paths remain relative (but the resolver will deny them separately).
fn normalize_without_io(path: &Path) -> PathBuf {
    use std::path::Component;

    let mut normalized = PathBuf::new();

    for component in path.components() {
        match component {
            // Root dir component (/ on Unix, C:\ on Windows) — always push.
            Component::RootDir | Component::Prefix(_) => {
                normalized.push(component.as_os_str());
            }
            // Current dir (.) — skip, it's a no-op.
            Component::CurDir => {}
            // Parent dir (..) — pop the last component if possible.
            // Clamped at root — can't go above it.
            Component::ParentDir => {
                normalized.pop();
            }
            // Normal component — push it.
            Component::Normal(name) => {
                normalized.push(name);
            }
        }
    }

    normalized
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::DirectoryPolicy;
    use std::collections::HashMap;
    use tempfile::TempDir;

    /// Build a DirectoryPolicy with an already-canonical path (for tests).
    fn make_dir_policy(path: PathBuf) -> DirectoryPolicy {
        DirectoryPolicy {
            path,
            owner: HashMap::new(),
            external: HashMap::new(),
        }
    }

    // ── normalize_without_io tests ─────────────────────────────────────────────

    #[test]
    fn test_normalize_removes_dot() {
        // Single dot (.) is a no-op and should be removed.
        let result = normalize_without_io(Path::new("/foo/./bar"));
        assert_eq!(result, PathBuf::from("/foo/bar"));
    }

    #[test]
    fn test_normalize_resolves_dotdot() {
        // Double dot (..) should pop the parent component.
        let result = normalize_without_io(Path::new("/foo/bar/../baz"));
        assert_eq!(result, PathBuf::from("/foo/baz"));
    }

    #[test]
    fn test_normalize_clamps_at_root() {
        // Too many .. components should not go above root.
        let result = normalize_without_io(Path::new("/foo/../../../../../../etc/passwd"));
        assert_eq!(result, PathBuf::from("/etc/passwd"));
    }

    #[test]
    fn test_normalize_multiple_dots() {
        // Multiple consecutive .. components should all be resolved.
        let result = normalize_without_io(Path::new("/a/b/c/../../d"));
        assert_eq!(result, PathBuf::from("/a/d"));
    }

    // ── PathGuard::find_directory tests ────────────────────────────────────────

    #[test]
    fn test_path_inside_allowed_dir_found() {
        // A file path inside an allowed directory should be found.
        let tmp = TempDir::new().unwrap();
        let dir_path = tmp.path().to_path_buf();

        // Create the file so canonicalize() works.
        let file_path = dir_path.join("hello.txt");
        std::fs::write(&file_path, "data").unwrap();

        // Canonicalize the directory manually (as PolicyConfig::validate() would).
        let canonical_dir = std::fs::canonicalize(&dir_path).unwrap();
        let policies = vec![make_dir_policy(canonical_dir)];
        let guard = PathGuard::new(&policies);

        let result = guard.find_directory(&file_path);
        assert!(result.is_some(), "file inside allowed dir should be found");
    }

    #[test]
    fn test_path_outside_all_dirs_not_found() {
        // A path outside all registered directories should return None.
        let tmp = TempDir::new().unwrap();
        let canonical_dir = std::fs::canonicalize(tmp.path()).unwrap();
        let policies = vec![make_dir_policy(canonical_dir)];
        let guard = PathGuard::new(&policies);

        // Use a different temp dir that is NOT in the policies.
        let other_tmp = TempDir::new().unwrap();
        let result = guard.find_directory(other_tmp.path());
        assert!(result.is_none(), "path outside allowed dirs should return None");
    }

    #[test]
    fn test_path_prefix_does_not_falsely_match() {
        // "/allowed" must NOT cover "/allowedBUT/file".
        // This is the critical component-boundary test.
        let tmp = TempDir::new().unwrap();
        let allowed = tmp.path().join("allowed");
        std::fs::create_dir(&allowed).unwrap();
        let allowed_but = tmp.path().join("allowedBUT");
        std::fs::create_dir(&allowed_but).unwrap();

        let canonical_allowed = std::fs::canonicalize(&allowed).unwrap();
        let policies = vec![make_dir_policy(canonical_allowed)];
        let guard = PathGuard::new(&policies);

        // A file inside "allowedBUT" should NOT match the "allowed" policy.
        let file_in_but = allowed_but.join("secret.txt");
        std::fs::write(&file_in_but, "data").unwrap();

        let result = guard.find_directory(&file_in_but);
        assert!(
            result.is_none(),
            "/allowedBUT/file must not match the /allowed policy"
        );
    }

    #[test]
    fn test_nonexistent_path_still_checked() {
        // A path that doesn't exist yet (new file being created) should still
        // be checked against the allowed directories using normalize_without_io().
        let tmp = TempDir::new().unwrap();
        let canonical_dir = std::fs::canonicalize(tmp.path()).unwrap();
        let policies = vec![make_dir_policy(canonical_dir)];
        let guard = PathGuard::new(&policies);

        // This file does NOT exist on disk — guard should still find the policy.
        let new_file = tmp.path().join("brand_new_file.rs");
        assert!(!new_file.exists(), "test setup: file should not exist yet");

        let result = guard.find_directory(&new_file);
        assert!(
            result.is_some(),
            "nonexistent file inside allowed dir should still match"
        );
    }

    #[test]
    fn test_dotdot_traversal_blocked_for_existing_paths() {
        // A path that uses .. to escape the allowed directory should be denied.
        // This tests the symlink/traversal attack surface.
        // For existing paths, canonicalize() resolves the real path.
        let tmp = TempDir::new().unwrap();
        let allowed = tmp.path().join("allowed");
        std::fs::create_dir(&allowed).unwrap();
        let sensitive = tmp.path().join("sensitive");
        std::fs::create_dir(&sensitive).unwrap();
        std::fs::write(sensitive.join("secret.txt"), "top secret").unwrap();

        let canonical_allowed = std::fs::canonicalize(&allowed).unwrap();
        let policies = vec![make_dir_policy(canonical_allowed)];
        let guard = PathGuard::new(&policies);

        // Attempt to traverse out of allowed via ..
        // e.g. /tmp/.../allowed/../sensitive/secret.txt
        let traversal = allowed.join("..").join("sensitive").join("secret.txt");
        let result = guard.find_directory(&traversal);
        assert!(
            result.is_none(),
            ".. traversal out of allowed dir must be blocked"
        );
    }

    #[test]
    fn test_multiple_directories_first_match_returned() {
        // When multiple policies are registered, find_directory returns the first match.
        let tmp = TempDir::new().unwrap();
        let dir_a = tmp.path().join("a");
        let dir_b = tmp.path().join("b");
        std::fs::create_dir(&dir_a).unwrap();
        std::fs::create_dir(&dir_b).unwrap();

        let canonical_a = std::fs::canonicalize(&dir_a).unwrap();
        let canonical_b = std::fs::canonicalize(&dir_b).unwrap();

        let policy_a = make_dir_policy(canonical_a.clone());
        let policy_b = make_dir_policy(canonical_b);
        let policies = vec![policy_a, policy_b];
        let guard = PathGuard::new(&policies);

        // A file inside dir_a should match the first policy (index 0).
        let file_a = dir_a.join("file.txt");
        std::fs::write(&file_a, "data").unwrap();

        let result = guard.find_directory(&file_a);
        assert!(result.is_some());
        assert_eq!(result.unwrap().path, canonical_a, "should match dir_a policy");
    }

    #[test]
    fn test_empty_policies_returns_none() {
        // With no policies registered, any path returns None.
        let guard = PathGuard::new(&[]);
        let result = guard.find_directory(Path::new("/some/path/file.txt"));
        assert!(result.is_none(), "empty policy list should deny all paths");
    }
}
