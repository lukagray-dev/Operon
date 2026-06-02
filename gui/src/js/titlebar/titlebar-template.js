/**
 * Titlebar Template
 * 
 * This module provides the HTML structure for the custom titlebar.
 * It's separated from the main titlebar controller for better maintainability
 * and follows the separation of concerns principle.
 * 
 * The template can be injected into the DOM programmatically or used
 * as a reference for direct HTML implementation.
 */

'use strict';

/**
 * Get the complete titlebar HTML structure
 * 
 * This function returns a string containing the full HTML markup
 * for the custom titlebar with all necessary elements and classes.
 * 
 * @returns {string} HTML string for the titlebar
 */
export function getTitlebarTemplate() {
    return `
        <div class="titlebar">
            <!-- LEFT SECTION: Logo, Navigation, Menu -->
            <div class="titlebar__left">
                <!-- Brand Logo -->
                <div class="titlebar__logo">
                    <img 
                        src="./assets/brand/operon.svg" 
                        alt="Operon Logo" 
                        class="titlebar__logo-icon"
                    />
                </div>

                <!-- Navigation Controls (Back/Forward) -->
                <nav class="titlebar__navigation">
                    <button 
                        id="titlebar-nav-back" 
                        class="titlebar__nav-btn" 
                        aria-label="Navigate back"
                        title="Go back"
                    >
                        <img 
                            src="./assets/icons/action/arrow-left.svg" 
                            alt="Back" 
                            class="titlebar__nav-icon"
                        />
                    </button>
                    <button 
                        id="titlebar-nav-forward" 
                        class="titlebar__nav-btn" 
                        aria-label="Navigate forward"
                        title="Go forward"
                    >
                        <img 
                            src="./assets/icons/action/arrow-right.svg" 
                            alt="Forward" 
                            class="titlebar__nav-icon"
                        />
                    </button>
                </nav>

                <!-- Menu Bar -->
                <div class="titlebar__menu">
                    <!-- Files Menu -->
                    <div class="titlebar__menu-item" data-menu="files">
                        <span>Files</span>
                        <div class="titlebar__dropdown">
                            <ul class="titlebar__dropdown-list">
                                <li>
                                    <button 
                                        id="menu-new-conversation" 
                                        class="titlebar__dropdown-item"
                                    >
                                        New conversation
                                    </button>
                                </li>
                                <li>
                                    <button 
                                        id="menu-settings" 
                                        class="titlebar__dropdown-item"
                                    >
                                        Settings
                                    </button>
                                </li>
                                <li>
                                    <div class="titlebar__dropdown-separator"></div>
                                </li>
                                <li>
                                    <button 
                                        id="menu-open-project" 
                                        class="titlebar__dropdown-item"
                                    >
                                        Open project
                                    </button>
                                </li>
                            </ul>
                        </div>
                    </div>

                    <!-- View Menu -->
                    <div class="titlebar__menu-item" data-menu="view">
                        <span>View</span>
                        <div class="titlebar__dropdown">
                            <ul class="titlebar__dropdown-list">
                                <li>
                                    <button 
                                        id="menu-reload" 
                                        class="titlebar__dropdown-item"
                                    >
                                        Reload
                                    </button>
                                </li>
                                <li>
                                    <button 
                                        id="menu-zoom-in" 
                                        class="titlebar__dropdown-item"
                                    >
                                        Zoom in
                                    </button>
                                </li>
                                <li>
                                    <button 
                                        id="menu-zoom-out" 
                                        class="titlebar__dropdown-item"
                                    >
                                        Zoom out
                                    </button>
                                </li>
                                <li>
                                    <button 
                                        id="menu-actual-size" 
                                        class="titlebar__dropdown-item"
                                    >
                                        Actual size
                                    </button>
                                </li>
                            </ul>
                        </div>
                    </div>

                    <!-- Window Menu -->
                    <div class="titlebar__menu-item" data-menu="window">
                        <span>Window</span>
                        <div class="titlebar__dropdown">
                            <ul class="titlebar__dropdown-list">
                                <li>
                                    <button 
                                        id="menu-close-window" 
                                        class="titlebar__dropdown-item"
                                    >
                                        Close window
                                    </button>
                                </li>
                                <li>
                                    <button 
                                        id="menu-exit" 
                                        class="titlebar__dropdown-item"
                                    >
                                        Exit
                                    </button>
                                </li>
                            </ul>
                        </div>
                    </div>

                    <!-- Help Menu -->
                    <div class="titlebar__menu-item" data-menu="help">
                        <span>Help</span>
                        <div class="titlebar__dropdown">
                            <ul class="titlebar__dropdown-list">
                                <li>
                                    <button 
                                        id="menu-documentation" 
                                        class="titlebar__dropdown-item"
                                    >
                                        Documentation
                                    </button>
                                </li>
                                <li>
                                    <button 
                                        id="menu-check-update" 
                                        class="titlebar__dropdown-item"
                                    >
                                        Check for update
                                    </button>
                                </li>
                                <li>
                                    <button 
                                        id="menu-report-bug" 
                                        class="titlebar__dropdown-item"
                                    >
                                        Report bug
                                    </button>
                                </li>
                                <li>
                                    <button 
                                        id="menu-about" 
                                        class="titlebar__dropdown-item"
                                    >
                                        About
                                    </button>
                                </li>
                                <li>
                                    <button 
                                        id="menu-follow-creator" 
                                        class="titlebar__dropdown-item"
                                    >
                                        Follow creator
                                    </button>
                                </li>
                                <li>
                                    <button 
                                        id="menu-see-repo" 
                                        class="titlebar__dropdown-item"
                                    >
                                        See repo
                                    </button>
                                </li>
                            </ul>
                        </div>
                    </div>
                </div>
            </div>

            <!-- RIGHT SECTION: Window Controls -->
            <div class="titlebar__right">
                <div class="titlebar__window-controls">
                    <!-- Minimize Button -->
                    <button 
                        id="titlebar-minimize" 
                        class="titlebar__control-btn" 
                        aria-label="Minimize window"
                        title="Minimize"
                    >
                        <img 
                            src="./assets/icons/action/minimize.svg" 
                            alt="Minimize" 
                            class="titlebar__control-icon"
                        />
                    </button>

                    <!-- Maximize/Unmaximize Button -->
                    <button 
                        id="titlebar-maximize" 
                        class="titlebar__control-btn" 
                        aria-label="Maximize window"
                        title="Maximize"
                    >
                        <img 
                            src="./assets/icons/action/maximize.svg" 
                            alt="Maximize" 
                            class="titlebar__control-icon titlebar__control-icon--maximize"
                        />
                        <img 
                            src="./assets/icons/action/unmaxmize.svg" 
                            alt="Restore" 
                            class="titlebar__control-icon titlebar__control-icon--unmaximize"
                        />
                    </button>

                    <!-- Close Button -->
                    <button 
                        id="titlebar-close" 
                        class="titlebar__control-btn titlebar__control-btn--close" 
                        aria-label="Close window"
                        title="Close"
                    >
                        <img 
                            src="./assets/icons/action/close.svg" 
                            alt="Close" 
                            class="titlebar__control-icon"
                        />
                    </button>
                </div>
            </div>
        </div>
    `;
}

/**
 * Inject the titlebar into the DOM
 * 
 * This function creates the titlebar element and inserts it
 * as the first child of the body element.
 * 
 * @returns {HTMLElement} The created titlebar element
 */
export function injectTitlebar() {
    const titlebarHtml = getTitlebarTemplate();
    const tempContainer = document.createElement('div');
    tempContainer.innerHTML = titlebarHtml;
    const titlebarElement = tempContainer.firstElementChild;
    
    // Insert at the beginning of body
    document.body.insertBefore(titlebarElement, document.body.firstChild);
    
    return titlebarElement;
}
