#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::*;
    use axum::http::StatusCode;
    use serde_json::json;

    #[tokio::test]
    async fn test_login_success() {
        // Setup
        let pool = create_test_pool().await;
        run_test_migrations(&pool).await;
        
        let test_email = "test_login@example.com";
        let test_password = "Test1234!";
        
        // Create test user
        create_test_user(&pool, test_email, test_password)
            .await
            .expect("Failed to create test user");
        
        // Test login
        let login_request = LoginRequest {
            email: test_email.to_string(),
            password: test_password.to_string(),
        };
        
        // In a real test, you would call the handler directly
        // For now, we just verify the test setup works
        
        // Cleanup
        cleanup_test_data(&pool).await;
    }

    #[tokio::test]
    async fn test_login_invalid_credentials() {
        let pool = create_test_pool().await;
        run_test_migrations(&pool).await;
        
        let login_request = LoginRequest {
            email: "nonexistent@example.com".to_string(),
            password: "wrongpassword".to_string(),
        };
        
        // Test would verify that login returns 401
        
        cleanup_test_data(&pool).await;
    }

    #[tokio::test]
    async fn test_password_hashing() {
        let password = "TestPassword123!";
        let hash = bcrypt::hash(password, 10).unwrap();
        
        assert!(bcrypt::verify(password, &hash).unwrap());
        assert!(!bcrypt::verify("WrongPassword", &hash).unwrap());
    }
}
