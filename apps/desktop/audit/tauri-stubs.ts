// Substitutos das APIs Tauri na auditoria visual (browser normal).
export const getCurrentWebview = () => ({ onDragDropEvent: async () => () => {} });
export const getCurrentWindow = () => ({ isFullscreen: async () => false, setFullscreen: async () => {} });
export const open = async () => null;
export const save = async () => null;
