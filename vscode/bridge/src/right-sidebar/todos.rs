//! Session Tasks Backend Commands for Right Sidebar in Bridge.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoItemDto {
    pub id: String,
    pub content: String,
    pub status: String,
    pub priority: String,
}

impl From<operon_rs::tools::TodoItem> for TodoItemDto {
    fn from(item: operon_rs::tools::TodoItem) -> Self {
        Self {
            id: item.id,
            content: item.content,
            status: format!("{:?}", item.status).to_lowercase(),
            priority: format!("{:?}", item.priority).to_lowercase(),
        }
    }
}

impl From<TodoItemDto> for operon_rs::tools::TodoItem {
    fn from(dto: TodoItemDto) -> Self {
        let status = match dto.status.to_lowercase().as_str() {
            "in_progress" | "inprogress" => operon_rs::tools::TodoStatus::InProgress,
            "completed" | "done" => operon_rs::tools::TodoStatus::Completed,
            _ => operon_rs::tools::TodoStatus::Pending,
        };

        let priority = match dto.priority.to_lowercase().as_str() {
            "high" => operon_rs::tools::TodoPriority::High,
            "low" => operon_rs::tools::TodoPriority::Low,
            _ => operon_rs::tools::TodoPriority::Medium,
        };

        Self {
            id: dto.id,
            content: dto.content,
            status,
            priority,
        }
    }
}

/// Retrieves the current list of todos for a session.
pub async fn get_session_todos(session_id: String) -> Result<Vec<TodoItemDto>, String> {
    if session_id.trim().is_empty() {
        return Ok(Vec::new());
    }

    let paths = operon_rs::config::OperonPaths::resolve().map_err(|e| e.to_string())?;
    let db_path = paths.session_db(&session_id);

    if !db_path.exists() {
        return Ok(Vec::new());
    }

    let store = operon_rs::session::store::SessionStore::open(&db_path)
        .await
        .map_err(|e| e.to_string())?;

    let items = store
        .load_todos(&session_id)
        .await
        .map_err(|e| e.to_string())?;
    Ok(items.into_iter().map(TodoItemDto::from).collect())
}

/// Updates the status of a specific todo item in a session.
pub async fn update_session_todo_status(
    session_id: String,
    todo_id: String,
    status: String,
) -> Result<Vec<TodoItemDto>, String> {
    if session_id.trim().is_empty() {
        return Err("Session ID cannot be empty".to_string());
    }

    let paths = operon_rs::config::OperonPaths::resolve().map_err(|e| e.to_string())?;
    let db_path = paths.session_db(&session_id);

    let store = operon_rs::session::store::SessionStore::open(&db_path)
        .await
        .map_err(|e| e.to_string())?;

    let mut items = store
        .load_todos(&session_id)
        .await
        .map_err(|e| e.to_string())?;

    let new_status = match status.to_lowercase().as_str() {
        "in_progress" | "inprogress" => operon_rs::tools::TodoStatus::InProgress,
        "completed" | "done" => operon_rs::tools::TodoStatus::Completed,
        _ => operon_rs::tools::TodoStatus::Pending,
    };

    let mut found = false;
    for item in &mut items {
        if item.id == todo_id {
            item.status = new_status;
            found = true;
            break;
        }
    }

    if !found {
        return Err(format!(
            "Todo with ID '{todo_id}' not found in session '{session_id}'"
        ));
    }

    store
        .save_todos(&session_id, &items)
        .await
        .map_err(|e| e.to_string())?;
    Ok(items.into_iter().map(TodoItemDto::from).collect())
}

/// Deletes a specific todo item from a session.
pub async fn delete_session_todo(
    session_id: String,
    todo_id: String,
) -> Result<Vec<TodoItemDto>, String> {
    if session_id.trim().is_empty() {
        return Err("Session ID cannot be empty".to_string());
    }

    let paths = operon_rs::config::OperonPaths::resolve().map_err(|e| e.to_string())?;
    let db_path = paths.session_db(&session_id);

    let store = operon_rs::session::store::SessionStore::open(&db_path)
        .await
        .map_err(|e| e.to_string())?;

    let mut items = store
        .load_todos(&session_id)
        .await
        .map_err(|e| e.to_string())?;
    items.retain(|item| item.id != todo_id);

    store
        .save_todos(&session_id, &items)
        .await
        .map_err(|e| e.to_string())?;
    Ok(items.into_iter().map(TodoItemDto::from).collect())
}

/// Creates a new todo item in a session.
pub async fn create_session_todo(
    session_id: String,
    content: String,
    priority: Option<String>,
) -> Result<Vec<TodoItemDto>, String> {
    if session_id.trim().is_empty() {
        return Err("Session ID cannot be empty".to_string());
    }

    let trimmed_content = content.trim();
    if trimmed_content.is_empty() {
        return Err("Todo content cannot be empty".to_string());
    }

    let paths = operon_rs::config::OperonPaths::resolve().map_err(|e| e.to_string())?;
    let db_path = paths.session_db(&session_id);

    let store = operon_rs::session::store::SessionStore::open(&db_path)
        .await
        .map_err(|e| e.to_string())?;

    let mut items = store
        .load_todos(&session_id)
        .await
        .map_err(|e| e.to_string())?;

    let next_id = items
        .iter()
        .filter_map(|i| i.id.parse::<usize>().ok())
        .max()
        .unwrap_or(0)
        + 1;

    let item_priority = match priority
        .as_deref()
        .unwrap_or("medium")
        .to_lowercase()
        .as_str()
    {
        "high" => operon_rs::tools::TodoPriority::High,
        "low" => operon_rs::tools::TodoPriority::Low,
        _ => operon_rs::tools::TodoPriority::Medium,
    };

    items.push(operon_rs::tools::TodoItem {
        id: next_id.to_string(),
        content: trimmed_content.to_string(),
        status: operon_rs::tools::TodoStatus::Pending,
        priority: item_priority,
    });

    store
        .save_todos(&session_id, &items)
        .await
        .map_err(|e| e.to_string())?;
    Ok(items.into_iter().map(TodoItemDto::from).collect())
}
