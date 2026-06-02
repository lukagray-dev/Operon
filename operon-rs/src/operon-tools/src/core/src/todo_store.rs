//! In-memory todo store for the agent's task list.
//!
//! One instance per agent session, owned by the `Dispatcher`.
//! Todos are session-scoped — they do not persist across sessions.
//! Compaction does NOT clear todos — the task plan survives summarization.
//!
//! The store is not designed for concurrent access — all methods take `&mut self`.
//! It is owned by the Dispatcher and accessed only during tool dispatch.

use crate::todo::{TodoItem, TodoPriority, TodoStatus};

/// In-memory store for the agent's todo list.
///
/// Manages a list of TodoItem objects with auto-incrementing IDs.
/// All methods take `&mut self` — not designed for concurrent access.
/// The store is owned by the Dispatcher and accessed during tool dispatch.
#[derive(Debug, Default)]
pub struct TodoStore {
    /// The list of todo items in insertion order.
    items: Vec<TodoItem>,
    /// Counter for auto-assigning unique IDs. Incremented on each create.
    next_id: u64,
}

impl TodoStore {
    /// Creates an empty store.
    ///
    /// # Example
    /// ```ignore
    /// let store = TodoStore::new();
    /// assert!(store.is_empty());
    /// ```
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new todo item and appends it to the list.
    ///
    /// Assigns a unique auto-incrementing ID (as a string: "1", "2", "3", ...).
    /// Status defaults to Pending, priority defaults to Medium if not provided.
    ///
    /// # Arguments
    /// - `content`: The task description (should be non-empty after trim).
    /// - `priority`: Optional priority level. Defaults to Medium if None.
    ///
    /// # Returns
    /// The created item (including its assigned `id`).
    ///
    /// # Example
    /// ```ignore
    /// let mut store = TodoStore::new();
    /// let item = store.create("Fix the login bug".to_string(), None);
    /// assert_eq!(item.id, "1");
    /// assert_eq!(item.status, TodoStatus::Pending);
    /// ```
    pub fn create(&mut self, content: String, priority: Option<TodoPriority>) -> TodoItem {
        self.next_id += 1;
        let item = TodoItem {
            id: self.next_id.to_string(),
            content,
            status: TodoStatus::Pending,
            priority: priority.unwrap_or_default(),
        };
        self.items.push(item.clone());
        item
    }

    /// Returns a snapshot of the current todo list.
    ///
    /// Items are returned in insertion order (the order they were created).
    /// This is a clone of the internal list — modifications to the returned
    /// vector do not affect the store.
    ///
    /// # Example
    /// ```ignore
    /// let mut store = TodoStore::new();
    /// store.create("Task 1".to_string(), None);
    /// store.create("Task 2".to_string(), None);
    /// let items = store.list();
    /// assert_eq!(items.len(), 2);
    /// ```
    pub fn list(&self) -> Vec<TodoItem> {
        self.items.clone()
    }

    /// Updates an existing todo item by id.
    ///
    /// Only fields wrapped in `Some` are updated — `None` means "no change".
    /// This allows partial updates: you can update just the status without
    /// changing content or priority.
    ///
    /// # Arguments
    /// - `id`: The ID of the item to update (as a string).
    /// - `content`: New content, or None to leave unchanged.
    /// - `status`: New status, or None to leave unchanged.
    /// - `priority`: New priority, or None to leave unchanged.
    ///
    /// # Returns
    /// - `Some(updated_item)` if the item was found and updated.
    /// - `None` if the id was not found.
    ///
    /// # Example
    /// ```ignore
    /// let mut store = TodoStore::new();
    /// let item = store.create("Task".to_string(), None);
    /// let updated = store.update(&item.id, None, Some(TodoStatus::InProgress), None);
    /// assert!(updated.is_some());
    /// assert_eq!(updated.unwrap().status, TodoStatus::InProgress);
    /// ```
    pub fn update(
        &mut self,
        id: &str,
        content: Option<String>,
        status: Option<TodoStatus>,
        priority: Option<TodoPriority>,
    ) -> Option<TodoItem> {
        let item = self.items.iter_mut().find(|i| i.id == id)?;
        if let Some(c) = content {
            item.content = c;
        }
        if let Some(s) = status {
            item.status = s;
        }
        if let Some(p) = priority {
            item.priority = p;
        }
        Some(item.clone())
    }

    /// Deletes a todo item by id.
    ///
    /// # Arguments
    /// - `id`: The ID of the item to delete (as a string).
    ///
    /// # Returns
    /// - `true` if the item was found and removed.
    /// - `false` if the id was not found.
    ///
    /// # Example
    /// ```ignore
    /// let mut store = TodoStore::new();
    /// let item = store.create("Task".to_string(), None);
    /// assert!(store.delete(&item.id));
    /// assert!(store.is_empty());
    /// ```
    pub fn delete(&mut self, id: &str) -> bool {
        let before = self.items.len();
        self.items.retain(|i| i.id != id);
        self.items.len() < before
    }

    /// Returns the number of items in the store.
    ///
    /// # Example
    /// ```ignore
    /// let mut store = TodoStore::new();
    /// store.create("Task 1".to_string(), None);
    /// store.create("Task 2".to_string(), None);
    /// assert_eq!(store.len(), 2);
    /// ```
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Returns `true` if the store has no items.
    ///
    /// # Example
    /// ```ignore
    /// let store = TodoStore::new();
    /// assert!(store.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_store_is_empty() {
        let store = TodoStore::new();
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
    }

    #[test]
    fn test_create_assigns_unique_ids() {
        let mut store = TodoStore::new();
        let item1 = store.create("Task 1".to_string(), None);
        let item2 = store.create("Task 2".to_string(), None);
        let item3 = store.create("Task 3".to_string(), None);

        assert_eq!(item1.id, "1");
        assert_eq!(item2.id, "2");
        assert_eq!(item3.id, "3");
        assert_eq!(store.len(), 3);
    }

    #[test]
    fn test_create_defaults() {
        let mut store = TodoStore::new();
        let item = store.create("Task".to_string(), None);

        assert_eq!(item.status, TodoStatus::Pending);
        assert_eq!(item.priority, TodoPriority::Medium);
    }

    #[test]
    fn test_create_with_priority() {
        let mut store = TodoStore::new();
        let item = store.create("Task".to_string(), Some(TodoPriority::High));

        assert_eq!(item.priority, TodoPriority::High);
    }

    #[test]
    fn test_list_returns_all_items() {
        let mut store = TodoStore::new();
        store.create("Task 1".to_string(), None);
        store.create("Task 2".to_string(), None);

        let items = store.list();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].content, "Task 1");
        assert_eq!(items[1].content, "Task 2");
    }

    #[test]
    fn test_update_status() {
        let mut store = TodoStore::new();
        let item = store.create("Task".to_string(), None);

        let updated = store.update(&item.id, None, Some(TodoStatus::InProgress), None);
        assert!(updated.is_some());
        assert_eq!(updated.unwrap().status, TodoStatus::InProgress);
    }

    #[test]
    fn test_update_content() {
        let mut store = TodoStore::new();
        let item = store.create("Old".to_string(), None);

        let updated = store.update(&item.id, Some("New".to_string()), None, None);
        assert!(updated.is_some());
        assert_eq!(updated.unwrap().content, "New");
    }

    #[test]
    fn test_update_priority() {
        let mut store = TodoStore::new();
        let item = store.create("Task".to_string(), Some(TodoPriority::Low));

        let updated = store.update(&item.id, None, None, Some(TodoPriority::High));
        assert!(updated.is_some());
        assert_eq!(updated.unwrap().priority, TodoPriority::High);
    }

    #[test]
    fn test_update_nonexistent_id() {
        let mut store = TodoStore::new();
        let result = store.update("99999", None, Some(TodoStatus::Completed), None);
        assert!(result.is_none());
    }

    #[test]
    fn test_delete_existing_item() {
        let mut store = TodoStore::new();
        let item = store.create("Task".to_string(), None);

        assert!(store.delete(&item.id));
        assert!(store.is_empty());
    }

    #[test]
    fn test_delete_nonexistent_id() {
        let mut store = TodoStore::new();
        assert!(!store.delete("99999"));
    }

    #[test]
    fn test_delete_specific_item() {
        let mut store = TodoStore::new();
        let item1 = store.create("Task 1".to_string(), None);
        let item2 = store.create("Task 2".to_string(), None);

        assert!(store.delete(&item1.id));
        assert_eq!(store.len(), 1);
        let items = store.list();
        assert_eq!(items[0].id, item2.id);
    }
}
