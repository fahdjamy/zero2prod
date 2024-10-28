use actix_web::cookie::Cookie;
use actix_web::http::header::ContentType;
use actix_web::HttpResponse;
use actix_web_flash_messages::IncomingFlashMessages;
use std::fmt::Write;

pub async fn login_form(flash_messages: IncomingFlashMessages) -> HttpResponse {
    // String implements std::fmt::Write. This means a String can be used as the destination for writeln!
    // Without use std::fmt::Write;, the compiler doesn't know that you intend to use the Write trait from the std::fmt module
    let mut error_html = String::new();
    // for m in flash_messages.iter().filter(|m| m.level() == Level::Error) {
    // Display all error messages
    for m in flash_messages.iter() {
        // The writeln! macro works by taking a destination and a formatted string.
        // It expects the destination to implement the std::fmt::Write trait.
        writeln!(error_html, "<p><i>{}</i></p>", m.content()).unwrap();
    }

    let mut response = HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta http-equiv="content-type" content="text/html; charset=utf-8">
    <title>Login</title>
</head>
<body>
    {error_html}
    <form action="/login" method="post">
        <label>Username
            <input
                type="text"
                placeholder="Enter Username"
                name="username"
            >
        </label>
        <label>Password
            <input
                type="password"
                placeholder="Enter Password"
                name="password"
            >
        </label>
        <button type="submit">Login</button>
    </form>
</body>
</html>"#,
        ));

    response
        .add_removal_cookie(&Cookie::new("_flash", ""))
        .unwrap();
    response
}
