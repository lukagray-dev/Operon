/**
 * Global JavaScript Tokens
 * 
 * This file contains all global constants, configuration values, and reusable
 * tokens that are shared across the Operon GUI application.
 * 
 * These tokens ensure consistency and maintainability across the codebase
 * by centralizing all configuration values in one place.
 */

'use strict';

/**
 * Color tokens - matches CSS custom properties for consistency
 * These colors are used throughout the application for theming
 */
export const COLORS = {
    // Primary brand colors
    BRAND_PRIMARY: '#2563eb',
    BRAND_SECONDARY: '#60a5fa',
    
    // Titlebar specific colors
    TITLEBAR_BG: '#191919',
    TITLEBAR_TEXT: '#e5e5e5',
    TITLEBAR_ICON: '#b3b3b3',
    TITLEBAR_ICON_HOVER: '#ffffff',
    TITLEBAR_HOVER_BG: '#2a2a2a',
    TITLEBAR_ACTIVE_BG: '#333333',
    
    // Window control specific colors
    WINDOW_CLOSE_HOVER: '#e81123',
    WINDOW_CLOSE_ACTIVE: '#c50f1f',
    
    // Background colors
    BG_PRIMARY: '#0d0d0d',
    BG_SECONDARY: '#1a1a1a',
    BG_TERTIARY: '#262626',
    
    // Text colors
    TEXT_PRIMARY: '#ffffff',
    TEXT_SECONDARY: '#a3a3a3',
    TEXT_TERTIARY: '#737373',
    
    // Border colors
    BORDER_PRIMARY: '#404040',
    BORDER_SECONDARY: '#333333',
};

/**
 * Spacing tokens - consistent spacing throughout the app
 * Values in pixels
 */
export const SPACING = {
    XS: 4,
    SM: 8,
    MD: 12,
    LG: 16,
    XL: 24,
    XXL: 32,
};

/**
 * Typography tokens - font sizes and weights
 */
export const TYPOGRAPHY = {
    FONT_FAMILY: {
        PRIMARY: 'Google Sans, -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif',
        MONO: 'Kode Mono, "Courier New", monospace',
    },
    FONT_SIZE: {
        XS: '11px',
        SM: '12px',
        MD: '13px',
        LG: '14px',
        XL: '16px',
        XXL: '18px',
    },
    FONT_WEIGHT: {
        REGULAR: 400,
        MEDIUM: 500,
        SEMIBOLD: 600,
        BOLD: 700,
    },
};

/**
 * Titlebar configuration
 */
export const TITLEBAR = {
    HEIGHT: 40, // Height in pixels
    LOGO_SIZE: 20, // Logo width/height in pixels
    ICON_SIZE: 16, // Icon width/height in pixels
    BUTTON_WIDTH: 46, // Window control button width
};

/**
 * Animation durations in milliseconds
 */
export const ANIMATION = {
    FAST: 100,
    NORMAL: 200,
    SLOW: 300,
};

/**
 * External URLs used in the application
 */
export const URLS = {
    DOCUMENTATION: 'https://github.com/lukagray-dev/Operon/tree/main/docs',
    REPORT_BUG: 'https://github.com/lukagray-dev/Operon/issues',
    REPOSITORY: 'https://github.com/lukagray-dev/Operon',
    CREATOR_INSTAGRAM: 'https://www.instagram.com/lukagray.official/',
};

/**
 * Z-index layers - ensures proper stacking order
 */
export const Z_INDEX = {
    BASE: 0,
    DROPDOWN: 1000,
    MODAL: 2000,
    TOOLTIP: 3000,
    NOTIFICATION: 4000,
};

/**
 * Breakpoints for responsive design (in pixels)
 */
export const BREAKPOINTS = {
    MOBILE: 480,
    TABLET: 768,
    DESKTOP: 1024,
    WIDE: 1440,
};

/**
 * Asset paths - centralized paths to assets
 */
export const ASSETS = {
    ICONS: {
        ACTION: './assets/icons/action',
        SIDEBAR: './assets/icons/sidebar',
        MAIN_CONTENT: './assets/icons/main-content',
    },
    BRAND: './assets/brand',
    FONTS: './assets/fonts',
};

/**
 * Window zoom levels for View menu
 */
export const ZOOM_LEVELS = {
    MIN: 0.5,
    DEFAULT: 1.0,
    MAX: 2.0,
    STEP: 0.1,
};
