use std::{env, io::Cursor, sync::Arc};

use axum::{
    extract::{DefaultBodyLimit, Multipart, State},
    http::{header, Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use image::{
    imageops::{self, FilterType},
    ImageFormat,
};
use time::{macros::format_description, UtcOffset};
use tokio::{
    fs::{self, File},
    io::AsyncWriteExt,
};
use tower_http::services::{ServeDir, ServeFile};
use tracing_subscriber::fmt::time::OffsetTime;
use uuid::Uuid;

struct AppState {
    storage_path: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // don't know why am not even using it kekw
    let offset = UtcOffset::from_hms(7, 0, 0).unwrap();
    let timer = OffsetTime::new(
        offset,
        format_description!("[year]-[month]-[day]T[hour]:[minute]:[second]"),
    );
    tracing_subscriber::fmt()
        .with_target(false)
        .with_timer(timer)
        .init();

    let storage_path = match env::var("STORAGE") {
        Ok(path) => path,
        Err(err) => {
            tracing::error!("{err}");
            std::process::exit(1);
        }
    };

    let shared_state = Arc::new(AppState { storage_path });

    fs::create_dir_all(format!("{}/preview", &shared_state.storage_path))
        .await
        .unwrap();

    let html_service = ServeDir::new("html").not_found_service(ServeFile::new("html/404.html"));
    let image_service = ServeDir::new(&shared_state.storage_path)
        .not_found_service(ServeFile::new("html/404.html"));

    // can use this but its not return error(statuscode 413) that I want
    // DefaultBodyLimit::max(1024 * 1024 * 10)
    let api = Router::new().route(
        "/upload",
        post(upload)
            .route_layer(DefaultBodyLimit::disable())
            .route_layer(middleware::from_fn(upload_middleware)),
    );

    let app = Router::new()
        .route("/lists", get(lists))
        .nest("/api", api)
        .nest_service("/static", image_service)
        .fallback_service(html_service)
        .with_state(shared_state);

    let addr = &"0.0.0.0:3000".parse().unwrap();
    let listener = match axum::Server::try_bind(addr) {
        Ok(listener) => listener,
        Err(err) => {
            tracing::error!("{err}");
            std::process::exit(1)
        }
    };

    tracing::info!("listening on port 3000");
    listener.serve(app.into_make_service()).await.unwrap();

    Ok(())
}

async fn upload_middleware<B>(request: Request<B>, next: Next<B>) -> Result<Response, StatusCode> {
    let content_length_str = request.headers().get(header::CONTENT_LENGTH).unwrap();
    let content_length: usize = content_length_str.to_str().unwrap().parse().unwrap();

    if content_length > 1024 * 1024 * 10 {
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
    }

    let response = next.run(request).await;

    Ok(response)
}

async fn upload(
    State(shared_state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    while let Some(field) = multipart.next_field().await.unwrap() {
        let content_type = field.content_type().unwrap().to_string();
        let data = field.bytes().await.unwrap();

        let format: Option<ImageFormat>;
        let mut extension: Option<String> = None;
        if !["image/png", "image/jpeg"].contains(&content_type.as_str()) {
            return (StatusCode::UNSUPPORTED_MEDIA_TYPE).into_response();
        } else {
            format = match content_type.as_str() {
                "image/png" => {
                    extension = Some("png".to_owned());
                    Some(ImageFormat::Png)
                }
                "image/jpeg" => {
                    extension = Some("jpeg".to_owned());
                    Some(ImageFormat::Jpeg)
                }
                _ => None,
            };
            if format.is_none() {
                return (StatusCode::UNSUPPORTED_MEDIA_TYPE).into_response();
            }
        }

        let uuid = Uuid::new_v4();
        {
            let name = format!(
                "{}/{}.{}",
                &shared_state.storage_path,
                uuid,
                extension.as_ref().unwrap()
            );
            let mut file = File::create(name).await.unwrap();
            for chunk in data.chunks(1024) {
                file.write_all(chunk).await.unwrap();
            }
        }
        {
            let name = format!(
                "{}/preview/{}.{}",
                &shared_state.storage_path,
                uuid,
                extension.as_ref().unwrap()
            );
            let cursor = Cursor::new(&data);
            let mut img_reader = image::io::Reader::new(cursor);
            img_reader.set_format(format.unwrap());
            let img = img_reader.decode().unwrap();

            // Define the target width
            let target_width = 240;
            // Calculate the corresponding height to maintain the aspect ratio
            let target_height =
                (target_width as f32 / img.width() as f32 * img.height() as f32) as u32;

            let img = imageops::resize(
                &img.to_rgba8(),
                target_width,
                target_height,
                FilterType::Lanczos3,
            );

            img.save(name).unwrap();
        }
    }

    (StatusCode::OK).into_response()
}

async fn lists(State(shared_state): State<Arc<AppState>>) -> impl IntoResponse {
    let mut paths = fs::read_dir(&shared_state.storage_path).await.unwrap();

    let mut body = String::new();
    while let Some(path) = paths.next_entry().await.unwrap() {
        if path.file_type().await.unwrap().is_dir() {
            continue;
        }

        body.push_str(
            format!(
                "<a href=\"/static/{name}\" target=\"_blank\"><img style=\"padding: 0.2rem;max-width: calc(100vw - 16px - 0.2rem);max-height: 135px;\" src=\"/static/preview/{name}\"/></a>",
                name = path.file_name().to_string_lossy()
            )
            .as_str(),
        );
    }

    (StatusCode::OK, [(header::CONTENT_TYPE, "text/html")], body).into_response()
}
