# Left Sidebar Components

This directory contains all components for the left sidebar navigation panel of the Operon GUI.

## Component Structure

```
sidebar.slint           # Main sidebar container (exports Sidebar component)
├── new-chat.slint      # "New chat" button at the top
├── search.slint        # "Search" button
├── plugins.slint       # "Plugins" button
├── projects.slint      # Projects section with expandable groups
│   └── conversation.slint  # Individual conversation item (reused)
├── chats.slint         # Standalone chats section
│   └── conversation.slint  # Individual conversation item (reused)
└── settings.slint      # "Settings" button at the bottom
```

## Usage Example

```slint
import { Sidebar } from "./ui/left-sidebar/sidebar.slint";

export component MainWindow inherits Window {
    // Sample data
    property <[{name: string, conversations: [string]}]> sample-projects: [
        {
            name: "Lunify",
            conversations: [
                "Hi",
                "You are working in this directory: 'D:\...",
                "Add CloudStream style streaming",
                "Fix FFI error handling",
                "Fix player bar position"
            ]
        },
        {
            name: "Operon",
            conversations: [
                "I was building an AI agent named Operon...",
                "Continue plan implementation",
                "Execute Prompt.md",
                "I was building an AI agent named Op...",
                "Replace markdown renderer"
            ]
        }
    ];
    
    property <[string]> sample-chats: [
        "Show more"
    ];
    
    HorizontalLayout {
        // Left sidebar
        Sidebar {
            projects: sample-projects;
            chats: sample-chats;
            
            // Handle callbacks
            new-chat-clicked => {
                debug("New chat clicked");
            }
            
            search-clicked => {
                debug("Search clicked");
            }
            
            plugins-clicked => {
                debug("Plugins clicked");
            }
            
            settings-clicked => {
                debug("Settings clicked");
            }
            
            project-conversation-clicked(proj-idx, conv-idx) => {
                debug("Project conversation clicked:", proj-idx, conv-idx);
            }
            
            chat-clicked(chat-idx) => {
                debug("Chat clicked:", chat-idx);
            }
            
            width-changed(new-width) => {
                debug("Sidebar width changed:", new-width);
            }
        }
        
        // Main content area
        Rectangle {
            background: #1a1a1a;
            
            Text {
                text: "Main Content Area";
                color: white;
            }
        }
    }
}
```

## Features

### Resizable Sidebar
- Drag the right edge of the sidebar to resize
- Minimum width: 180px
- Maximum width: 400px
- Default width: 240px

### Top Actions
- **New Chat**: Primary action button with distinct styling
- **Search**: Quick access to search functionality
- **Plugins**: Access to plugins/extensions

### Projects Section
- Displays projects with expandable/collapsible groups
- Each project shows its associated conversations
- Click project header to expand/collapse
- Conversations are indented under their project

### Chats Section
- Shows standalone conversations not associated with any project
- Empty state message when no chats exist
- Scrollable when content overflows

### Bottom Actions
- **Settings**: Always visible at the bottom for easy access

## Styling

All components use the centralized design tokens from `ui/shared/tokens.slint`:

- **Colors**: Consistent with the dark theme
- **Typography**: Google Sans font family
- **Spacing**: Standardized padding and gaps
- **Animations**: Smooth transitions (150-200ms)
- **Hover States**: Subtle background changes

## Accessibility

- All interactive elements have proper touch areas
- Text overflow is handled with ellipsis
- Keyboard navigation support (via Slint framework)
- Clear visual hierarchy with proper contrast

## Integration with Main Window

To integrate the sidebar into the main window (`ui/window.slint`):

```slint
import { Sidebar } from "./left-sidebar/sidebar.slint";

export component OperonWindow inherits Window {
    // ... existing properties ...
    
    HorizontalLayout {
        // Add sidebar
        sidebar := Sidebar {
            y: titlebar.height; // Position below titlebar
            height: parent.height - titlebar.height;
            
            // Bind to data from Rust backend
            projects: /* bind to backend data */;
            chats: /* bind to backend data */;
            
            // Wire up callbacks to Rust
            new-chat-clicked => { /* call Rust */ }
            // ... other callbacks ...
        }
        
        // Main content area
        Rectangle {
            // ... existing content ...
        }
    }
    
    // ... existing titlebar ...
}
```

## Data Structure

### Projects
```rust
// Rust side
struct Project {
    name: String,
    conversations: Vec<String>,
}
```

### Chats
```rust
// Rust side
type Chats = Vec<String>;
```

## Notes

- The sidebar uses `ScrollView` for the middle section to handle overflow
- All SVG icons are loaded from `assets/icons/sidebar/`
- The resize handle provides visual feedback on hover
- Smooth animations enhance the user experience
- Components are modular and can be used independently
