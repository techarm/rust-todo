use std::env;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::{Json, Router};
use axum::extract::Extension;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use dotenv::dotenv;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::handlers::{all_todo, create_todo, delete_todo, find_todo, update_todo};
use crate::repositories::{TodoRepository, TodoRepositoryForDb};

mod handlers;
mod repositories;

#[tokio::main]
async fn main() {
    // loggingの初期化
    let log_level = env::var("RUST_LOG").unwrap_or("info".to_string());
    env::set_var("RUST_LOG", log_level);
    tracing_subscriber::fmt::init();
    dotenv().ok();

    let database_url = &env::var("DATABASE_URL").expect("undefined [DATABASE_URL]");
    tracing::debug!("start connect database...");
    let pool = PgPool::connect(database_url)
        .await
        .expect(&format!("fail connect database, url is [{}]", database_url));
    let repository = TodoRepositoryForDb::new(pool.clone());
    let app = create_app(repository);

    // run our app with hyper, listening globally on port 3000
    // let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    tracing::debug!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

fn create_app<T: TodoRepository>(repository: T) -> Router {
    Router::new()
        .route("/", get(root))
        .route("/users", post(create_user))
        .route("/todos", post(create_todo::<T>).get(all_todo::<T>))
        .route(
            "/todos/:id",
            get(find_todo::<T>)
                .delete(delete_todo::<T>)
                .patch(update_todo::<T>),
        )
        .layer(Extension(Arc::new(repository)))
}

async fn root() -> &'static str {
    "Hello, World!"
}

async fn create_user(Json(payload): Json<CreateUser>) -> impl IntoResponse {
    let user = User {
        id: 1337,
        username: payload.username,
    };
    (StatusCode::CREATED, Json(user))
}

#[derive(Serialize, Deserialize, Debug, PartialOrd, PartialEq)]
struct CreateUser {
    username: String,
}

#[derive(Serialize, Deserialize, Debug, PartialOrd, PartialEq)]
struct User {
    id: u64,
    username: String,
}

#[cfg(test)]
mod test {
    use axum::body::Body;
    use axum::http::{Method, Request};
    use axum::response::Response;
    use hyper::header;
    use tower::ServiceExt;

    use crate::repositories::{CreateTodo, Todo, TodoRepositoryForMemory};

    use super::*;

    fn build_req_with_json(path: &str, method: Method, json_body: String) -> Request<Body> {
        Request::builder()
            .uri(path)
            .method(method)
            .header(header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
            .body(Body::from(json_body))
            .unwrap()
    }

    fn build_todo_req_with_empty(method: Method, path: &str) -> Request<Body> {
        Request::builder()
            .uri(path)
            .method(method)
            .body(Body::empty())
            .unwrap()
    }

    async fn res_to_todo(res: Response) -> Todo {
        let bytes = hyper::body::to_bytes(res.into_body()).await.unwrap();
        let body: String = String::from_utf8(bytes.to_vec()).unwrap();
        let todo: Todo = serde_json::from_str(&body)
            .expect(&format!("cannot convert Todo instance. body: {}", body));
        todo
    }

    #[tokio::test]
    async fn should_return_hello_world() {
        let repository = TodoRepositoryForMemory::new();
        let req = Request::builder().uri("/").body(Body::empty()).unwrap();
        let res = create_app(repository).oneshot(req).await.unwrap();
        let bytes = hyper::body::to_bytes(res.into_body()).await.unwrap();
        let body: String = String::from_utf8(bytes.to_vec()).unwrap();
        assert_eq!(body, "Hello, World!");
    }

    #[tokio::test]
    async fn should_return_user_data() {
        let req = Request::builder()
            .uri("/users")
            .method(Method::POST)
            .header(header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
            .body(Body::from(r#"{ "username": "techarm" }"#))
            .unwrap();
        let repository = TodoRepositoryForMemory::new();
        let res = create_app(repository).oneshot(req).await.unwrap();
        let bytes = hyper::body::to_bytes(res.into_body()).await.unwrap();
        let body: String = String::from_utf8(bytes.to_vec()).unwrap();
        let user: User = serde_json::from_str(&body).expect("cannot convert User instance.");
        assert_eq!(
            user,
            User {
                id: 1337,
                username: "techarm".to_string(),
            }
        )
    }

    #[tokio::test]
    async fn should_created_todo() {
        let expected = Todo::new(1, "should_return_created_todo".to_string());

        let repository = TodoRepositoryForMemory::new();
        let req = build_req_with_json(
            "/todos",
            Method::POST,
            r#"{ "text": "should_return_created_todo"}"#.to_string(),
        );
        let res = create_app(repository).oneshot(req).await.unwrap();
        let todo = res_to_todo(res).await;
        assert_eq!(expected, todo);
    }

    #[tokio::test]
    async fn should_find_todo() {
        let expected = Todo::new(1, "should_find_todo".to_string());

        let repository = TodoRepositoryForMemory::new();
        repository
            .create(CreateTodo::new("should_find_todo".to_string()))
            .await
            .expect("failed create todo");
        let req = build_todo_req_with_empty(Method::GET, "/todos/1");
        let res = create_app(repository).oneshot(req).await.unwrap();
        let todo = res_to_todo(res).await;
        assert_eq!(expected, todo);
    }

    #[tokio::test]
    async fn should_get_all_todos() {
        let expected = Todo::new(1, "should_get_all_todos".to_string());

        let repository = TodoRepositoryForMemory::new();
        repository
            .create(CreateTodo::new("should_get_all_todos".to_string()))
            .await
            .expect("failed create todo");

        let req = build_todo_req_with_empty(Method::GET, "/todos");
        let res = create_app(repository).oneshot(req).await.unwrap();
        let bytes = hyper::body::to_bytes(res.into_body()).await.unwrap();
        let body: String = String::from_utf8(bytes.to_vec()).unwrap();
        let todos: Vec<Todo> = serde_json::from_str(&body)
            .expect(&format!("cannot convert Todo lis instance. body: {}", body));
        assert_eq!(vec![expected], todos);
    }

    #[tokio::test]
    async fn should_update_todo() {
        let expected = Todo::new(1, "should_update_todo".to_string());

        let repository = TodoRepositoryForMemory::new();
        repository
            .create(CreateTodo::new("before_update_todo".to_string()))
            .await
            .expect("failed create todo");

        let req = build_req_with_json(
            "/todos/1",
            Method::PATCH,
            r#"{ "text": "should_update_todo", "completed": false }"#.to_string(),
        );
        let res = create_app(repository).oneshot(req).await.unwrap();
        let todo = res_to_todo(res).await;
        assert_eq!(expected, todo);
    }

    #[tokio::test]
    async fn should_delete_todo() {
        let repository = TodoRepositoryForMemory::new();
        repository
            .create(CreateTodo::new("should_delete_todo".to_string()))
            .await
            .expect("failed create todo");

        let req = build_todo_req_with_empty(Method::DELETE, "/todos/1");
        let res = create_app(repository).oneshot(req).await.unwrap();
        assert_eq!(StatusCode::NO_CONTENT, res.status());
    }
}
