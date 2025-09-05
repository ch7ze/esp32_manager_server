// ============================================================================
// BACKEND INTEGRATION TESTS ENTRY POINT
// Organisiert alle Backend Integration Tests
// ============================================================================

mod backend;

// Re-export Integration Tests
pub use backend::integration::auth_tests;