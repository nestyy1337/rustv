mod common;
use backend::{repositories::users::UserRepository, shared::test_utils::setup_test_app};

#[tokio::test]
async fn test_find_user_by_name_empty() {
    let (_addr, pool) = setup_test_app().await.unwrap();

    let user = backend::shared::test_utils::test_user();

    let found =
        backend::repositories::users::UserRepository::find_by_username(&pool, &user.username).await;
    assert!(found.is_ok());
    let found = found.unwrap();
    assert!(found.is_some());
    let found = found.unwrap();
    assert_eq!(found.id, user.id);
    assert_eq!(found.username, user.username);
}

#[tokio::test]
async fn test_find_user_by_name_valid() {
    let (_addr, pool) = setup_test_app().await.unwrap();

    let user = backend::shared::test_utils::test_user();

    let _ = backend::repositories::users::UserRepository::add_user(
        &user,
        "hunter42".to_string(),
        &pool,
    )
    .await;

    let found =
        backend::repositories::users::UserRepository::find_by_username(&pool, &user.username).await;

    assert!(found.is_ok());
    let found = found.unwrap();
    assert!(found.is_some());
    let found = found.unwrap();
    assert_eq!(found.id, user.id);
    assert_eq!(found.username, user.username);
}

#[tokio::test]
async fn test_add_user_duplicate_username() {
    let (_addr, pool) = setup_test_app().await.unwrap();
    let mut user = backend::shared::test_utils::test_user();
    user.username = "duplicate_user".to_string();
    user.email = "duplicate@gmail.com".to_string();
    let result1 = backend::repositories::users::UserRepository::add_user(
        &user,
        "hunter42".to_string(),
        &pool,
    )
    .await;

    let mut duplicate_user = backend::shared::test_utils::test_user();
    duplicate_user.username = "duplicate_user".to_string();
    duplicate_user.email = "duplicated2@gmail.com".to_string();
    let result2 = backend::repositories::users::UserRepository::add_user(
        &duplicate_user,
        "hunter42".to_string(),
        &pool,
    )
    .await;

    let mut duplicate_user2 = backend::shared::test_utils::test_user();
    duplicate_user2.username = "duplicate_user1".to_string();
    duplicate_user2.email = "duplicate@gmail.com".to_string();
    let result3 = backend::repositories::users::UserRepository::add_user(
        &duplicate_user2,
        "hunter42".to_string(),
        &pool,
    )
    .await;

    assert!(result1.is_ok());
    assert!(result2.is_err());
    assert!(result3.is_err());
}
