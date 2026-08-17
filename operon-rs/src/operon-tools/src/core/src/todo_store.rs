//! In-memory todo store for the agent's task list.
//!
//! One instance per agent session, owned by the `Dispatcher`.
//! Todos are session-scoped — they do not persist across sessions.
//! Compaction does NOT clear todos — the task plan survives summarization.
//!
//! The store is not designed for concurrent access — all methods take `&mut self`.
//! It is owned by the Dispatcher and accessed only during tool dispatch.

use serde::{Deserialize, Serialize};

use crate::todo::{TodoItem, TodoPriority, TodoStatus};

/// In-memory store for the agent's todo list.
///
/// Manages a list of `TodoItem` objects with auto-incrementing numeric IDs ("1", "2", ...).
/// Methods taking `&mut self` mutate the internal list.
///
/// # Session Persistence
/// The store can be serialized/deserialized to JSON, allowing `SessionStore`
/// to preserve tasks across multiple conversation turns in the same session.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TodoStore {
    /// The list of todo items in insertion order.
    items: Vec<TodoItem>,
    /// Counter for auto-assigning unique IDs. Incremented on each create.
    next_id: u64,
}

impl TodoStore {
    /// Creates an empty store with `next_id` starting at 0.
    ///
    /// # Example
    /// ```ignore
    /// let store = TodoStore::new();
    /// assert!(store.is_empty());
    /// ```
    pub fn new() -> Self {
        Self::default()
    }

    /// Reconstructs a `TodoStore` from an existing list of `TodoItem` items.
    ///
    /// Hey friend! When resuming a previous session, we load the saved items from disk.
    /// This constructor calculates the highest existing numeric ID so any new tasks created
    /// afterwards get a unique, higher ID (e.g. if we load items "1" and "2", the next created
    /// task will automatically receive ID "3"!).
    ///
    /// # Arguments
    /// - `items`: The vector of saved `TodoItem` items to initialize the store with.
    pub fn from_items(items: Vec<TodoItem>) -> Self {
        // Hey buddy! We look at all existing item IDs, parse them as integers,
        // and find the maximum one. If there are no items or IDs aren't numeric, we default to 0.
        let max_id = items
            .iter()
            .filter_map(|item| item.id.parse::<u64>().ok())
            .max()
            .unwrap_or(0);

        Self {
            items,
            next_id: max_id,
        }
    }

    /// Replaces the current items in the store and recalculates `next_id`.
    ///
    /// Useful when restoring or syncing session todos from persistent storage.
    pub fn set_items(&mut self, items: Vec<TodoItem>) {
        let max_id = items
            .iter()
            .filter_map(|item| item.id.parse::<u64>().ok())
            .max()
            .unwrap_or(0);

        self.items = items;
        self.next_id = max_id;
    }

    /// Returns a borrowed slice of all todo items in insertion order.
    pub fn items(&self) -> &[TodoItem] {
        &self.items
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

    /// Creates multiple todo items in a single atomic batch and appends them to the list.
    ///
    /// Assigns sequential auto-incrementing IDs to each created item.
    ///
    /// # Arguments
    /// - `items`: Vector of `(content, priority)` pairs to create.
    ///
    /// # Returns
    /// A vector containing all newly created `TodoItem` records in order.
    pub fn create_many(&mut self, items: Vec<(String, Option<TodoPriority>)>) -> Vec<TodoItem> {
        let mut created = Vec::with_capacity(items.len());
        for (content, priority) in items {
            created.push(self.create(content, priority));
        }
        created
    }

    /// Updates multiple todo items in a single batch.
    ///
    /// For each update tuple `(id, content, status, priority)`, finds the matching
    /// item and applies any non-None fields.
    ///
    /// # Returns
    /// `(updated_items, not_found_ids)`
    pub fn update_many(
        &mut self,
        updates: Vec<(String, Option<String>, Option<TodoStatus>, Option<TodoPriority>)>,
    ) -> (Vec<TodoItem>, Vec<String>) {
        let mut updated = Vec::new();
        let mut not_found = Vec::new();

        for (id, content, status, priority) in updates {
            match self.update(&id, content, status, priority) {
                Some(item) => updated.push(item),
                None => not_found.push(id),
            }
        }

        (updated, not_found)
    }

    /// Deletes multiple todo items by their IDs in a single batch.
    ///
    /// # Arguments
    /// - `ids`: Slice of ID strings to remove.
    ///
    /// # Returns
    /// `(deleted_ids, not_found_ids)`
    pub fn delete_many(&mut self, ids: &[String]) -> (Vec<String>, Vec<String>) {
        let mut deleted = Vec::new();
        let mut not_found = Vec::new();

        for id in ids {
            if self.delete(id) {
                deleted.push(id.clone());
            } else {
                not_found.push(id.clone());
            }
        }

        (deleted, not_found)
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
    fn test_from_items_and_id_continuation() {
        // Hey friend! Let's test that from_items properly calculates next_id from loaded tasks.
        let items = vec![
            TodoItem {
                id: "1".to_string(),
                content: "First task".to_string(),
                status: TodoStatus::Completed,
                priority: TodoPriority::High,
            },
            TodoItem {
                id: "5".to_string(),
                content: "Fifth task".to_string(),
                status: TodoStatus::Pending,
                priority: TodoPriority::Medium,
            },
        ];

        let mut store = TodoStore::from_items(items);
        assert_eq!(store.len(), 2);
        assert_eq!(store.items().len(), 2);

        // When creating a new task, its ID should continue right after the highest existing ID (5 -> 6)!
        let new_item = store.create("Sixth task".to_string(), None);
        assert_eq!(new_item.id, "6");
        assert_eq!(store.len(), 3);
    }

    #[test]
    fn test_set_items_updates_state() {
        let mut store = TodoStore::new();
        store.create("Initial".to_string(), None);

        let new_items = vec![TodoItem {
            id: "10".to_string(),
            content: "Restored item".to_string(),
            status: TodoStatus::InProgress,
            priority: TodoPriority::Low,
        }];

        store.set_items(new_items);
        assert_eq!(store.len(), 1);
        assert_eq!(store.items()[0].content, "Restored item");

        let next = store.create("Next item".to_string(), None);
        assert_eq!(next.id, "11");
    }

    #[test]
    fn test_json_serialization_roundtrip() {
        // Test that TodoStore can be serialized to JSON and deserialized back identically.
        let mut original = TodoStore::new();
        original.create("Task A".to_string(), Some(TodoPriority::High));
        original.create("Task B".to_string(), Some(TodoPriority::Low));

        let json = serde_json::to_string(&original).expect("Serialization failed");
        let restored: TodoStore = serde_json::from_str(&json).expect("Deserialization failed");

        assert_eq!(original, restored);
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

    #[test]
    fn test_create_many_assigns_sequential_ids() {
        let mut store = TodoStore::new();
        let items = store.create_many(vec![
            ("Task 1".to_string(), Some(TodoPriority::High)),
            ("Task 2".to_string(), None),
            ("Task 3".to_string(), Some(TodoPriority::Low)),
        ]);

        assert_eq!(items.len(), 3);
        assert_eq!(items[0].id, "1");
        assert_eq!(items[0].priority, TodoPriority::High);
        assert_eq!(items[1].id, "2");
        assert_eq!(items[1].priority, TodoPriority::Medium);
        assert_eq!(items[2].id, "3");
        assert_eq!(items[2].priority, TodoPriority::Low);
        assert_eq!(store.len(), 3);
    }

    #[test]
    fn test_update_many_handles_mixed_results() {
        let mut store = TodoStore::new();
        let items = store.create_many(vec![
            ("Task 1".to_string(), None),
            ("Task 2".to_string(), None),
        ]);

        let (updated, not_found) = store.update_many(vec![
            (items[0].id.clone(), Some("Task 1 Renamed".to_string()), Some(TodoStatus::Completed), None),
            ("9999".to_string(), None, Some(TodoStatus::InProgress), None),
        ]);

        assert_eq!(updated.len(), 1);
        assert_eq!(updated[0].content, "Task 1 Renamed");
        assert_eq!(updated[0].status, TodoStatus::Completed);
        assert_eq!(not_found, vec!["9999".to_string()]);
    }

    #[test]
    fn test_delete_many_removes_matching_items() {
        let mut store = TodoStore::new();
        let items = store.create_many(vec![
            ("Task 1".to_string(), None),
            ("Task 2".to_string(), None),
            ("Task 3".to_string(), None),
        ]);

        let (deleted, not_found) = store.delete_many(&[items[0].id.clone(), items[2].id.clone(), "missing".to_string()]);
        assert_eq!(deleted.len(), 2);
        assert_eq!(deleted, vec![items[0].id.clone(), items[2].id.clone()]);
        assert_eq!(not_found, vec!["missing".to_string()]);
        assert_eq!(store.len(), 1);
        assert_eq!(store.list()[0].id, items[1].id);
    }
}

