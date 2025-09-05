// Helper function for generating unique event IDs
export function generateEventId() {
    return Date.now().toString(36) + Math.random().toString(36).substring(2, 5);
}
//# sourceMappingURL=events.js.map