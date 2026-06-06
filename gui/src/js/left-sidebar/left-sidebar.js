// Import the settings panel so we can open it when the user clicks Settings
import { openSettings } from '../settings/settings-panel.js';

/**
 * Left Sidebar Component
 * 
 * This module handles all functionality for the left sidebar including:
 * - Action button handlers (New chat, Search, Plugins, Settings)
 * - Section collapse/expand (Projects, Chats)
 * - Project collapse/expand with nested chats
 * - Chat item selection and navigation
 * - Sidebar resizing functionality
 * - Dynamic rendering of projects and chats
 * 
 * The sidebar follows a hierarchical structure:
 * - Projects section contains multiple projects
 * - Each project can contain multiple chats
 * - Chats section contains standalone chats not associated with any project
 */

'use strict';

/**
 * LeftSidebarController class
 * 
 * Manages all left sidebar interactions including navigation, filtering,
 * and dynamic content rendering. This class follows the single responsibility
 * principle by separating concerns into focused private methods.
 */
class LeftSidebarController {
    /**
     * Constructor - Initializes the left sidebar controller
     * 
     * Sets up:
     * - Current width tracking for resize
     * - Active chat/section tracking
     * - Mock data for demonstration (will be replaced with real data)
     * - Event listeners for all interactive elements
     */
    constructor() {
        // Current sidebar width (for resizing)
        this.currentWidth = 240; // Default width in pixels
        
        // Track if currently resizing
        this.isResizing = false;
        
        // Track active chat ID
        this.activeChatId = null;
        
        // Track collapsed sections
        this.collapsedSections = new Set(); // Set of section IDs that are collapsed
        
        // Track collapsed projects
        this.collapsedProjects = new Set(); // Set of project IDs that are collapsed
        
        // Mock data for demonstration
        // In production, this would come from backend/state management
        this.mockData = this.generateMockData();
        
        // Initialize the sidebar
        this.init();
    }

    /**
     * Initialize all sidebar functionality
     * 
     * This is the main entry point that sets up all event listeners
     * and renders initial content.
     */
    init() {
        try {
            // Set up all event listeners
            this.initEventListeners();
            
            // Render initial content
            this.renderProjects();
            this.renderChats();
            
            console.log('Left sidebar initialized successfully');
        } catch (error) {
            console.error('Failed to initialize left sidebar:', error);
        }
    }

    /**
     * Generate initial empty data structure for projects and chats
     * 
     * @returns {Object} Initial empty data structure
     */
    generateMockData() {
        return {
            projects: [],
            chats: []
        };
    }

    /**
     * Initialize all event listeners
     * 
     * Sets up listeners for:
     * - Top action buttons (New chat, Search, Plugins)
     * - Bottom settings button
     * - Section toggles (Projects, Chats)
     * - Resize handle
     */
    initEventListeners() {
        // Top action buttons
        this.setupActionButtons();
        
        // Bottom settings button
        this.setupSettingsButton();
        
        // Section toggles
        this.setupSectionToggles();
        
        // Resize functionality
        this.setupResizeHandle();
    }

    /* ========================================================================
       ACTION BUTTON HANDLERS
       ======================================================================== */

    /**
     * Setup top action button listeners
     * 
     * Handles clicks on:
     * - New chat button
     * - Search button
     * - Plugins button
     */
    setupActionButtons() {
        const newChatBtn = document.getElementById('sidebar-new-chat');
        const searchBtn = document.getElementById('sidebar-search');
        const pluginsBtn = document.getElementById('sidebar-plugins');

        if (newChatBtn) {
            newChatBtn.addEventListener('click', () => {
                console.log('New chat clicked');
                this.handleNewChat();
            });
        }

        if (searchBtn) {
            searchBtn.addEventListener('click', () => {
                console.log('Search clicked');
                this.handleSearch();
            });
        }

        if (pluginsBtn) {
            pluginsBtn.addEventListener('click', () => {
                console.log('Plugins clicked');
                this.handlePlugins();
            });
        }
    }

    /**
     * Setup bottom settings button listener
     */
    setupSettingsButton() {
        const settingsBtn = document.getElementById('sidebar-settings');

        if (settingsBtn) {
            settingsBtn.addEventListener('click', () => {
                console.log('Settings clicked');
                this.handleSettings();
            });
        }
    }

    /**
     * Handle new chat action
     * 
     * Creates a new chat session. In production, this would:
     * - Create a new chat via backend API
     * - Navigate to the new chat view
     * - Update the sidebar to show the new chat
     */
    handleNewChat() {
        console.log('Creating new chat...');
        if (window.sessionManager) {
            window.sessionManager.startNewChat();
        }
    }

    /**
     * Handle search action
     * 
     * Opens search interface. In production, this would:
     * - Show a search input overlay
     * - Allow searching through all chats
     * - Filter and highlight matching results
     */
    handleSearch() {
        // TODO: Implement search functionality
        console.log('Opening search...');
        alert('Search functionality will be implemented here');
    }

    /**
     * Handle plugins action
     * 
     * Opens plugins management interface. In production, this would:
     * - Show available plugins
     * - Allow enabling/disabling plugins
     * - Configure plugin settings
     */
    handlePlugins() {
        // TODO: Implement plugins interface
        console.log('Opening plugins...');
        alert('Plugins functionality will be implemented here');
    }

    /**
     * Handle settings action
     * 
     * Opens settings dialog. In production, this would:
     * - Show application settings
     * - Allow configuring preferences
     * - Manage account settings
     */
    handleSettings() {
        // Open the settings popup dialog — only the X button closes it
        openSettings();
    }

    /* ========================================================================
       SECTION TOGGLE HANDLERS
       ======================================================================== */

    /**
     * Setup section toggle listeners
     * 
     * Handles collapse/expand for Projects and Chats sections
     */
    setupSectionToggles() {
        const sectionToggles = document.querySelectorAll('.left-sidebar__section-toggle');

        sectionToggles.forEach(toggle => {
            toggle.addEventListener('click', (e) => {
                e.stopPropagation();
                
                const sectionId = toggle.getAttribute('data-section');
                this.toggleSection(sectionId);
            });
        });
    }

    /**
     * Toggle section collapse/expand
     * 
     * @param {string} sectionId - The ID of the section to toggle
     */
    toggleSection(sectionId) {
        const toggle = document.querySelector(`[data-section="${sectionId}"]`);
        const content = document.querySelector(`[data-section-content="${sectionId}"]`);

        if (!toggle || !content) return;

        // Toggle collapsed state
        const isCollapsed = this.collapsedSections.has(sectionId);

        if (isCollapsed) {
            // Expand section
            this.collapsedSections.delete(sectionId);
            toggle.classList.remove('collapsed');
            content.classList.remove('collapsed');
        } else {
            // Collapse section
            this.collapsedSections.add(sectionId);
            toggle.classList.add('collapsed');
            content.classList.add('collapsed');
        }
    }

    /* ========================================================================
       PROJECT RENDERING AND HANDLERS
       ======================================================================== */

    /**
     * Render all projects and their nested chats
     * 
     * Dynamically creates the DOM structure for the Projects section
     * including all projects and their associated chats.
     */
    renderProjects() {
        const projectsContent = document.querySelector('[data-section-content="projects"]');
        if (!projectsContent) return;

        // Clear existing content
        projectsContent.innerHTML = '';

        // Render each project
        this.mockData.projects.forEach(project => {
            const projectElement = this.createProjectElement(project);
            projectsContent.appendChild(projectElement);
        });
    }

    /**
     * Create DOM element for a project
     * 
     * @param {Object} project - Project data object
     * @returns {HTMLElement} The project DOM element
     */
    createProjectElement(project) {
        // Create project container
        const projectDiv = document.createElement('div');
        projectDiv.className = 'left-sidebar__project';
        projectDiv.setAttribute('data-project-id', project.id);

        // Create project header
        const headerDiv = document.createElement('div');
        headerDiv.className = 'left-sidebar__project-header';

        const toggleBtn = document.createElement('button');
        toggleBtn.className = 'left-sidebar__project-toggle';
        toggleBtn.setAttribute('aria-label', `Toggle ${project.name} project`);

        // Folder icon
        const folderImg = document.createElement('img');
        folderImg.src = './assets/icons/sidebar/folder.svg';
        folderImg.alt = 'Folder';
        folderImg.className = 'left-sidebar__project-icon';

        // Project name
        const nameSpan = document.createElement('span');
        nameSpan.className = 'left-sidebar__project-name';
        nameSpan.textContent = project.name;

        // Chevron icon (on the right)
        const chevronImg = document.createElement('img');
        chevronImg.src = './assets/icons/sidebar/chevron-down.svg';
        chevronImg.alt = 'Toggle';
        chevronImg.className = 'left-sidebar__project-chevron';

        // Assemble toggle button (icon, name, then chevron on right)
        toggleBtn.appendChild(folderImg);
        toggleBtn.appendChild(nameSpan);
        toggleBtn.appendChild(chevronImg);
        headerDiv.appendChild(toggleBtn);
        projectDiv.appendChild(headerDiv);

        // Create project chats container
        const chatsDiv = document.createElement('div');
        chatsDiv.className = 'left-sidebar__project-chats';

        // Check if project should be collapsed
        const isCollapsed = this.collapsedProjects.has(project.id);
        if (isCollapsed) {
            toggleBtn.classList.add('collapsed');
            chatsDiv.classList.add('collapsed');
        }

        // Render project chats
        project.chats.forEach(chat => {
            const chatElement = this.createChatElement(chat);
            chatsDiv.appendChild(chatElement);
        });

        projectDiv.appendChild(chatsDiv);

        // Add click listener for project toggle
        toggleBtn.addEventListener('click', (e) => {
            e.stopPropagation();
            this.toggleProject(project.id);
        });

        return projectDiv;
    }

    /**
     * Toggle project collapse/expand
     * 
     * @param {string} projectId - The ID of the project to toggle
     */
    toggleProject(projectId) {
        const projectElement = document.querySelector(`[data-project-id="${projectId}"]`);
        if (!projectElement) return;

        const toggle = projectElement.querySelector('.left-sidebar__project-toggle');
        const chatsContainer = projectElement.querySelector('.left-sidebar__project-chats');

        if (!toggle || !chatsContainer) return;

        // Toggle collapsed state
        const isCollapsed = this.collapsedProjects.has(projectId);

        if (isCollapsed) {
            // Expand project
            this.collapsedProjects.delete(projectId);
            toggle.classList.remove('collapsed');
            chatsContainer.classList.remove('collapsed');
        } else {
            // Collapse project
            this.collapsedProjects.add(projectId);
            toggle.classList.add('collapsed');
            chatsContainer.classList.add('collapsed');
        }
    }

    /* ========================================================================
       CHAT RENDERING AND HANDLERS
       ======================================================================== */

    /**
     * Render standalone chats (not under any project)
     * 
     * Dynamically creates the DOM structure for the Chats section
     * with standalone chats.
     */
    renderChats() {
        if (window.sessionManager) {
            window.sessionManager.loadSessionsList();
        }
    }

    /**
     * Create DOM element for a chat
     * 
     * @param {Object} chat - Chat data object
     * @returns {HTMLElement} The chat DOM element
     */
    createChatElement(chat) {
        const chatBtn = document.createElement('button');
        chatBtn.className = 'left-sidebar__chat-item';
        chatBtn.setAttribute('aria-label', `Open chat: ${chat.title}`);
        chatBtn.setAttribute('data-chat-id', chat.id);

        const chatText = document.createElement('span');
        chatText.className = 'left-sidebar__chat-text';
        chatText.textContent = chat.title;

        chatBtn.appendChild(chatText);

        // Add click listener for chat selection
        chatBtn.addEventListener('click', (e) => {
            e.stopPropagation();
            this.selectChat(chat.id);
        });

        return chatBtn;
    }

    /**
     * Select a chat
     * 
     * Updates UI to show the selected chat as active and
     * loads the chat content.
     * 
     * @param {string} chatId - The ID of the chat to select
     */
    selectChat(chatId) {
        // Deselect previous active chat
        if (this.activeChatId) {
            const prevActive = document.querySelector(`[data-chat-id="${this.activeChatId}"]`);
            if (prevActive) {
                prevActive.classList.remove('active');
            }
        }

        // Select new chat
        const newActive = document.querySelector(`[data-chat-id="${chatId}"]`);
        if (newActive) {
            newActive.classList.add('active');
            this.activeChatId = chatId;
            
            console.log(`Selected chat: ${chatId}`);
            if (window.sessionManager) {
                window.sessionManager.selectSession(chatId);
            }
        }
    }

    /**
     * Load chat content
     * 
     * Loads the content of the selected chat into the main content area.
     * In production, this would fetch chat history from the backend.
     * 
     * @param {string} chatId - The ID of the chat to load
     */
    loadChatContent(chatId) {
        // TODO: Implement chat content loading
        console.log(`Loading chat content for: ${chatId}`);
        // This would typically:
        // 1. Fetch chat history from backend
        // 2. Render messages in main content area
        // 3. Set up input handlers for new messages
    }

    /* ========================================================================
       RESIZE FUNCTIONALITY
       ======================================================================== */

    /**
     * Setup resize handle functionality
     * 
     * Allows user to drag the right edge of the sidebar to resize it.
     * Enforces minimum and maximum width constraints.
     */
    setupResizeHandle() {
        const resizeHandle = document.querySelector('.left-sidebar__resize-handle');
        const sidebar = document.querySelector('.left-sidebar');

        if (!resizeHandle || !sidebar) return;

        let startX = 0;
        let startWidth = 0;

        /**
         * Handle mousedown on resize handle
         * Initiates resize operation
         */
        const handleMouseDown = (e) => {
            e.preventDefault();
            
            this.isResizing = true;
            startX = e.clientX;
            startWidth = sidebar.offsetWidth;

            // Add resizing class for visual feedback
            resizeHandle.classList.add('resizing');
            document.body.style.cursor = 'col-resize';
            document.body.style.userSelect = 'none';

            // Add global listeners for drag operation
            document.addEventListener('mousemove', handleMouseMove);
            document.addEventListener('mouseup', handleMouseUp);
        };

        /**
         * Handle mousemove during resize
         * Updates sidebar width
         */
        const handleMouseMove = (e) => {
            if (!this.isResizing) return;

            const deltaX = e.clientX - startX;
            const newWidth = startWidth + deltaX;

            // Enforce min/max constraints
            const minWidth = 180; // --sidebar-min-width
            const maxWidth = 400; // --sidebar-max-width

            if (newWidth >= minWidth && newWidth <= maxWidth) {
                sidebar.style.width = `${newWidth}px`;
                this.currentWidth = newWidth;
            }
        };

        /**
         * Handle mouseup to finish resize
         * Cleans up event listeners and state
         */
        const handleMouseUp = () => {
            if (!this.isResizing) return;

            this.isResizing = false;

            // Remove resizing class
            resizeHandle.classList.remove('resizing');
            document.body.style.cursor = '';
            document.body.style.userSelect = '';

            // Remove global listeners
            document.removeEventListener('mousemove', handleMouseMove);
            document.removeEventListener('mouseup', handleMouseUp);

            console.log(`Sidebar resized to: ${this.currentWidth}px`);
        };

        // Attach mousedown listener to resize handle
        resizeHandle.addEventListener('mousedown', handleMouseDown);
    }

    /* ========================================================================
       PUBLIC API METHODS
       ======================================================================== */

    /**
     * Refresh projects list
     * 
     * Re-renders the projects section with updated data.
     * Call this when projects data changes.
     * 
     * @param {Array} projects - Updated projects data
     */
    refreshProjects(projects) {
        if (projects) {
            this.mockData.projects = projects;
        }
        this.renderProjects();
    }

    /**
     * Refresh chats list
     * 
     * Re-renders the chats section with updated data.
     * Call this when chats data changes.
     * 
     * @param {Array} chats - Updated chats data
     */
    refreshChats(chats) {
        if (chats) {
            this.mockData.chats = chats;
        }
        this.renderChats();
    }

    /**
     * Add a new chat to a project
     * 
     * @param {string} projectId - The ID of the project
     * @param {Object} chat - The chat object to add
     */
    addChatToProject(projectId, chat) {
        const project = this.mockData.projects.find(p => p.id === projectId);
        if (project) {
            project.chats.push(chat);
            this.renderProjects();
        }
    }

    /**
     * Add a standalone chat
     * 
     * @param {Object} chat - The chat object to add
     */
    addStandaloneChat(chat) {
        this.mockData.chats.push(chat);
        this.renderChats();
    }

    /**
     * Get currently active chat ID
     * 
     * @returns {string|null} The active chat ID or null if none selected
     */
    getActiveChatId() {
        return this.activeChatId;
    }
}

/**
 * Initialize the left sidebar when DOM is ready
 * 
 * This creates a single instance of the LeftSidebarController
 * and makes it globally accessible for debugging and external use.
 */
let leftSidebarController = null;

if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', () => {
        leftSidebarController = new LeftSidebarController();
        window.leftSidebarController = leftSidebarController;
    });
} else {
    // DOM is already loaded
    leftSidebarController = new LeftSidebarController();
    window.leftSidebarController = leftSidebarController;
}

// Export for potential use in other modules
export default LeftSidebarController;
