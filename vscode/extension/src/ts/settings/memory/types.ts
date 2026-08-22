// Types for the Memory settings panel.

/** A single memory entry returned by the backend. */
export interface MemoryEntry {
  id: string;
  content: string;
  tags: string[];
  created_at: string;
  updated_at: string;
}

/** Response from memory_list: a page of entries plus the store total. */
export interface MemoryListResponse {
  memories: MemoryEntry[];
  total: number;
}
