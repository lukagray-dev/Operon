// Appearance Settings TypeScript Interfaces matching Slint 1:1

export interface AppearanceSettings {
  selected_theme: number; // 0 = Operon Dark, 1 = Midnight OLED, 2 = GitHub Dark, 3 = Tokyo Night
  selected_ui_scale: number; // 0 = 80%, 1 = 100%, 2 = 120%, 3 = 140%, 4 = 160%
  compact_mode: boolean;
  smooth_animations: boolean;
  selected_thinking_orb: number; // 0 = Breathing Aurora, 1 = Composing Prism, 2 = Solving Helix
  selected_ui_font: number; // 0 = Open Sans, 1 = Inter, 2 = Roboto
  selected_assistant_font: number; // 0 = Literata, 1 = Lora, 2 = Merriweather
  selected_code_font: number; // 0 = Kode Mono, 1 = JetBrains Mono, 2 = Fira Code
  code_block_theme: number; // 0 = GitHub Dark, 1 = Midnight OLED, 2 = Tokyo Night, 3 = Monokai
  show_line_numbers: boolean;
  highlight_inline_code: boolean;
  table_theme: number; // 0 = GitHub Dark, 1 = Modern Minimal, 2 = Zebra Striped, 3 = Boxed Grid
}
