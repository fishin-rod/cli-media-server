//! BlueBird
// use my api template to make a server and allow a client(s) to connect
// move back id generation to a server?
use std::collections::HashMap;

use axum::{
    body::{Body, to_bytes},
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post, delete, patch},
    Json, Router,
    http::request::Parts,
};

//use http::request::Parts;
use hyper::Request;
use serde_json::json;
use sqlx::{postgres::PgRow, FromRow, PgPool, Row};
use serde::{Deserialize, Serialize};
use rand::{distributions::Alphanumeric, Rng};

const ADMIN: &str = "nNuMPs82ERXOwJ4zvSxA";
const COMMENT_POST: &str = "/POSTS/COMMENT";
const LIKE_POST: &str = "/POSTS/LIKE";
const DISLIKE_POST: &str = "/POSTS/DISLIKE";
const LIKE_COMMENT: &str = "/COMMENTS/LIKE";
const DISLIKE_COMMENT: &str = "/COMMENTS/DISLIKE";

const FEED: &str = "/FEED";

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
struct SelfUser {
    id: String, 
    name: String,
    password: String,
    friends: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
struct User {
    id: String,
    name: String,
    friends: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
struct Post {
    id: String,
    user_id: String,
    title: String,
    body: String,
    date: String, // Account for timezones: D/M/YYYY H:MM:SS
    likes: i32,
    dislikes: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
struct Comment {
    id: String, 
    user_id: String,
    post_id: String,
    body: String,
    date: String,
    likes: i32,
    dislikes: i32,
}

#[derive(Debug, Serialize, Deserialize)]
struct Login {
    name: String,
    password: String,
}


#[derive(Clone)]
struct DB {
    pool: PgPool,
}

pub enum Response<T> {
    Success(),
    Return(Json<T>),
    Error(StatusCode, Json<serde_json::Value>),
}

impl From<SelfUser> for User {
    fn from(s: SelfUser) -> Self {
        User {
            id: s.id,
            name: s.name,
            friends: s.friends,
        }
    }
}

impl<T: Serialize> IntoResponse for Response<T> {
    fn into_response(self) -> axum::response::Response {
        match self {
            Response::Success() => (StatusCode::OK).into_response(),
            Response::Return(data) => (StatusCode::OK, data).into_response(),
            Response::Error(status, error) => (status, error).into_response(),
        }
    }
}

#[shuttle_runtime::main]
async fn main(
    #[shuttle_shared_db::Postgres] pool: PgPool
) ->  shuttle_axum::ShuttleAxum {
    sqlx::migrate!()
        .run(&pool)
        .await
        .expect("Failed to run migrations");
    let state = DB { pool };

    let router = Router::new()
        .route("/stop/:id", get(stop))
        .route("/posts/", get(posts).post(new_post))
        .route("/users/:id", get(get_user).patch(edit_user).delete(delete_user))
        .route("/users/", get(users).post(new_user))
        .route("/posts/:id", get(get_post).patch(edit_post).delete(delete_post))
        .route("/login/", post(login))
        .route("/friends/:id", post(add_friend).get(get_friends).delete(remove_friend))
        .with_state(state);

    Ok(router.into())
}    

async fn stop(Path(id): Path<String>) -> impl IntoResponse {
    if id == ADMIN {
        std::process::exit(0);
        //(StatusCode::OK, "Goodbye!")
    } else {
        (StatusCode::UNAUTHORIZED, "You must have admin privileges to stop the program!")
    }
}

async fn posts(State(state): State<DB>, parts: Parts) -> Response<Vec<Post>> {
    if !validate_key(parts, State(state.clone())).await {
        return Response::Error(StatusCode::UNAUTHORIZED, Json(json!({ "error": "Unauthorized" })));
    }
    let posts = sqlx::query_as::<_, Post>("SELECT * FROM posts").fetch_all(&state.pool).await.unwrap();
    println!("posts: {:#?}", posts);
    Response::Return(Json(posts))
}

async fn users(State(state): State<DB>, parts: Parts) -> Response<Vec<User>> {
    if !validate_key(parts, State(state.clone())).await {
        return Response::Error(StatusCode::UNAUTHORIZED, Json(json!({ "error": "Unauthorized" })));
    }

    // Fetch all users from the users table
    let users = sqlx::query(
        "SELECT id, name FROM users"
    )
    .fetch_all(&state.pool)
    .await
    .unwrap();

    // Prepare a vector to store the users along with their friends
    let mut users_with_friends = Vec::new();
    
    for row in users {
        let user_id: String = row.get("id");
        let name: String = row.get("name");

        // Fetch friend IDs for the current user
        let friend_ids: Vec<String> = sqlx::query(
            r#"
            SELECT friend_id
            FROM friends
            WHERE user_id = $1
            "#,
        )
        .bind(&user_id)
        .fetch_all(&state.pool)
        .await
        .unwrap()
        .into_iter()
        .map(|record| record.get::<String, _>("friend_id"))
        .collect();

        // Create the user with the fetched friend IDs
        users_with_friends.push(User {
            id: user_id,
            name,
            friends: friend_ids,
        });
    }

    Response::Return(Json(users_with_friends))
}

async fn new_post(State(state): State<DB>, req: Request<Body>) -> Response<Post> {
    let (parts, body) = req.into_parts();
    if !validate_key(parts, State(state.clone())).await {
        return Response::Error(StatusCode::UNAUTHORIZED, Json(json!({ "error": "Unauthorized" })));
    }
    let body_bytes = to_bytes(body, usize::MAX).await.unwrap();
    let post: Post = match serde_json::from_slice(&body_bytes) {
        Ok(post) => post,
        Err(err) => { return Response::Error(StatusCode::BAD_REQUEST, Json(json!({ "error": "Invalid JSON", "details": err.to_string() })));}
    };
    let post = Post {
        id: gen_id(30),
        user_id: post.user_id.clone(),
        title: post.title.clone(),
        body: post.body.clone(),
        date: post.date, 
        likes: 0,
        dislikes: 0,
    };
    let _ = sqlx::query(
        "INSERT INTO posts (id, user_id, title, body, date, likes, dislikes) VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(&post.id)
    .bind(&post.user_id)
    .bind(&post.title)
    .bind(&post.body)
    .bind(&post.date)
    .bind(post.likes)
    .bind(post.dislikes)
    .execute(&state.pool)
    .await
    .unwrap();
    println!("x: {:#?}", post);
    Response::Return(Json(post))
}

async fn new_user(State(state): State<DB>, Json(user): Json<Login>) -> Response<SelfUser> {
    let user = SelfUser {
        id: gen_id(30),
        name: user.name.clone(),
        password: user.password.clone(),
        friends: Vec::new(),
    };
    let check = sqlx::query(
        "SELECT * FROM users WHERE name = $1",
    ).bind(&user.name)
    .fetch_all(&state.pool).await.unwrap();
    if !check.is_empty() {
        return Response::Error(StatusCode::CONFLICT, Json(json!({ "error": "User already exists" })));
    }
    let _ = sqlx::query(
        "INSERT INTO users (id, name, password) VALUES ($1, $2, $3)",
    )
    .bind(&user.id)
    .bind(&user.name)
    .bind(&user.password)
    .execute(&state.pool)
    .await
    .unwrap();
    Response::Return(Json(user))
}

async fn login(State(state): State<DB>, Json(input): Json<Login>) -> Response<SelfUser> {
    let user = sqlx::query(
        "SELECT * FROM users WHERE name = $1",
    ).bind(&input.name).fetch_one(&state.pool).await.unwrap();

    let user_id: String = user.get("id");
    let name: String = user.get("name");
    let password: String = user.get("password");

    if name != input.name || password != input.password {
        return Response::Error(StatusCode::UNAUTHORIZED, Json(json!({ "error": "Invalid username or password" })));
    }

    let friends = sqlx::query(
        r#"
        SELECT friend_id
        FROM friends
        WHERE user_id = $1
        "#,
    )
    .bind(&user_id)
    .fetch_all(&state.pool)
    .await
    .unwrap()
    .into_iter()
    .map(|record| record.get::<String, _>("friend_id"))
    .collect();

    let user = SelfUser {
        id: user_id,
        name,
        password,
        friends,
    };

    Response::Return(Json(user))
}

async fn edit_post(Path(id): Path<String>, State(state): State<DB>, req: Request<Body>) -> Response<Post> {
    let (parts, body) = req.into_parts();
    if !validate_key(parts, State(state.clone())).await {
        return Response::Error(StatusCode::UNAUTHORIZED, Json(json!({ "error": "Unauthorized" })));
    }
    let body_bytes = to_bytes(body, usize::MAX).await.unwrap();
    let post: Post = match serde_json::from_slice(&body_bytes) {
        Ok(post) => post,
        Err(err) => { return Response::Error(StatusCode::BAD_REQUEST, Json(json!({ "error": "Invalid JSON", "details": err.to_string() })));}
    };
    let _ = sqlx::query(
        "UPDATE posts SET title = $3, body = $5 WHERE id = $1 RETURNING *",
    )
    .bind(id)
    .bind(&post.title)
    .bind(&post.body)
    .execute(&state.pool)
    .await
    .unwrap();

    Response::Success()
}

async fn edit_user(Path(id): Path<String>, State(state): State<DB>, req: Request<Body>) -> Response<SelfUser> {
    let (parts, body) = req.into_parts();
    if !validate_key(parts, State(state.clone())).await {
        return Response::Error(StatusCode::UNAUTHORIZED, Json(json!({ "error": "Unauthorized" })));
    }
    let body_bytes = to_bytes(body, usize::MAX).await.unwrap();
    let user: SelfUser = match serde_json::from_slice(&body_bytes) {
        Ok(user) => user,
        Err(err) => { return Response::Error(StatusCode::BAD_REQUEST, Json(json!({ "error": "Invalid JSON", "details": err.to_string() })));}
    };
    let _ = sqlx::query(
        "UPDATE users SET name = $2, password = $3 WHERE id = $1 RETURNING *",
    )
    .bind(id)
    .bind(&user.name)
    .bind(&user.password)
    .execute(&state.pool)
    .await
    .unwrap();

    Response::Success()
}

async fn add_friend(Path(id): Path<String>, State(state): State<DB>, parts: Parts) -> Response<User> {
    if !validate_key(parts, State(state.clone())).await {
        return Response::Error(StatusCode::UNAUTHORIZED, Json(json!({ "error": "Unauthorized" })));
    }
    sqlx::query(
        "INSERT INTO friends (user_id, friend_id) VALUES ($1, $2) ON CONFLICT DO NOTHING"
    )
    .bind(&id)
    .bind(&id)
    .execute(&state.pool)
    .await.unwrap();


    Response::Success()
}

async fn remove_friend(Path(id): Path<String>, State(state): State<DB>, parts: Parts) -> Response<User> {
    if !validate_key(parts, State(state.clone())).await {
        return Response::Error(StatusCode::UNAUTHORIZED, Json(json!({ "error": "Unauthorized" })));
    }
    sqlx::query(
        r#"
        DELETE FROM friends
        WHERE (user_id = $1 AND friend_id = $2)
           OR (user_id = $2 AND friend_id = $1)
        "#
    )
    .bind(&id)
    .bind(&id)
    .execute(&state.pool)
    .await.unwrap();

    Response::Success()
}

//fix
async fn get_friends(Path(id): Path<String>, State(state): State<DB>, parts: Parts) -> Response<User> {
    if !validate_key(parts, State(state.clone())).await {
        return Response::Error(StatusCode::UNAUTHORIZED, Json(json!({ "error": "Unauthorized" })));
    }
    let x = sqlx::query_as::<_, User>(
        "SELECT * FROM users WHERE id = ANY($1)",
    ).bind(&id)
    .fetch_one(&state.pool).await;

    match x {
        Ok(user) => Response::Return(Json(user)),
        Err(err) => {
            eprintln!("Database error: {:?}", err);
            Response::Error(StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": "Database error" })))
        }
    }
}

async fn delete_post(Path(id): Path<String>, State(state): State<DB>, parts: Parts) -> Response<Post> {//use id to only delete posts from id
    if !validate_key(parts, State(state.clone())).await {
        return Response::Error(StatusCode::UNAUTHORIZED, Json(json!({ "error": "Unauthorized" })));
    }
    let _ = sqlx::query(
        "DELETE FROM posts WHERE id = $1",
    )
    .bind(&id)
    .execute(&state.pool)
    .await
    .unwrap();

    Response::Success()
}

async fn delete_user(Path(id): Path<String>, State(state): State<DB>, parts: Parts) -> Response<User> {
    if !validate_key(parts, State(state.clone())).await {
        return Response::Error(StatusCode::UNAUTHORIZED, Json(json!({ "error": "Unauthorized" })));
    }
    let _ = sqlx::query(
        "DELETE FROM users WHERE id = $1",
    )
    .bind(&id)
    .execute(&state.pool)
    .await
    .unwrap();

    Response::Success()
}

//fix headersr
// perfect fucntion
async fn get_post(Path(id): Path<String>, State(state): State<DB>, parts: Parts) -> Response<Post>  {
    if !validate_key(parts, State(state.clone())).await {
        return Response::Error(StatusCode::UNAUTHORIZED, Json(json!({ "error": "Unauthorized" })));
    }
    let post = sqlx::query_as::<_, Post>("SELECT * FROM posts WHERE id = $1").bind(id).fetch_one(&state.pool).await;
    if post.is_err() {
        return Response::Error(StatusCode::NOT_FOUND, Json(json!({ "error": "Post not found" })));
    }
    Response::Return(Json(post.unwrap()))
}

async fn get_user(Path(id): Path<String>, State(state): State<DB>, parts: Parts) -> Response<SelfUser> {
    // Dont return as much if they are not themselves
    if !validate_key(parts, State(state.clone())).await {
        return Response::Error(StatusCode::UNAUTHORIZED, Json(json!({ "error": "Unauthorized" })));
    }
    let mut row: PgRow;
    let user = sqlx::query(
        "SELECT id, name, password FROM users WHERE id = $1",
    )
    .bind(&id)
    .fetch_one(&state.pool)
    .await;
    if user.is_err() {
        return Response::Error(StatusCode::NOT_FOUND, Json(json!({ "error": "User not found" })));
    }
    else {
        row = user.unwrap();
    }

    let user_id: String = row.get("id");
    let name: String = row.get("name");
    let password: String = row.get("password");

    let friend_ids: Vec<String> = sqlx::query(
        r#"
        SELECT friend_id
        FROM friends
        WHERE user_id = $1
        "#,
    )
    .bind(&id)
    .fetch_all(&state.pool)
    .await
    .unwrap()
    .into_iter()
    .map(|record| record.get::<String, _>("friend_id"))
    .collect();

    if user_id == id {
        let user = SelfUser {
            id: user_id,
            name,
            password,
            friends: friend_ids,
        };   
        Response::Return(Json(user))
    }
    //maek safe later
    else {
        let user = SelfUser {
            id: user_id,
            name,
            password,
            friends: friend_ids,
        };
        Response::Return(Json(user))
    }
}

async fn validate_key(key: Parts, State(state): State<DB>) -> bool {
    let key = key.headers.get("Authorization").unwrap().to_str().unwrap();
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM users WHERE id = $1)",
    ).bind(key).fetch_one(&state.pool).await.unwrap_or(false);
    exists
}

fn gen_id(len: usize) -> String {
    let rng = rand::thread_rng();
    let key: String = rng.sample_iter(&Alphanumeric).take(len).map(char::from).collect();

    key
}


