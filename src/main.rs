extern crate hyper;
extern crate hyper_native_tls;
#[macro_use]
extern crate lazy_static;
extern crate convenience;

use std::io;
use std::io::{Read, Write};

use hyper::header::{Headers, Host, Referer, ContentType, Location};
use hyper::mime::{Mime, TopLevel, SubLevel};
use hyper::uri::RequestUri;
use hyper::net::HttpsConnector;
use hyper::client::{Client, Body, RedirectPolicy};
use hyper::server::{Server, Request, Response};

lazy_static! {
    static ref PROTO: String = std::env::args().skip(1).next().expect("No protocol argument");
    static ref SITE: String = std::env::args().skip(2).next().expect("No site argument");
}

fn main() {
    println!("Hello, world!");
    Server::http("127.0.0.1:8081").unwrap().handle(handler).unwrap();
}

fn handler(mut s_req: Request, mut s_res: Response) {
    let uri = get_url(&s_req);

    if uri.contains("service-worker.js") {
        send_serviceworker(s_res);
        return;
    }

    let up_res = get_upstream_data(&mut s_req, uri);
    send_response(up_res, s_res);
}

fn is_html(headers: &Headers) -> bool {
    if let Some(ContentType(content_type)) = headers.get::<ContentType>().cloned() {
        match content_type {
            Mime(TopLevel::Text, SubLevel::Html, _) => true,
            _ => false,
        }
    } else {
        false
    }
}

fn get_url(req: &Request) -> String {
    let mut uri = match req.uri {
        RequestUri::AbsolutePath(ref uri) => format!("{}://{}{}", &*PROTO, &*SITE, uri),
        _ => panic!(),
    };

    // Wrts requires https for login page but http for the rest
    if &uri == "http://wrts.nl/signin" || &uri == "http://wrts.nl/signout" {
        uri = uri.replace("http://", "https://");
    }

    if !uri.ends_with(".jpg") && !uri.ends_with(".png") && !uri.contains(".js?ver=") &&
       !uri.ends_with(".js") && !uri.contains(".css?ver=") && !uri.ends_with(".css") &&
       !uri.contains(".svg?ver=") && !uri.ends_with(".svg") ||
       uri.ends_with("service-worker.js") {
        println!("GET {}", uri);
    }

    uri
}

fn send_serviceworker(mut res: Response) {
    println!("Send serviceworker");
    let mut sw = convenience::read_file("src/serviceworker.js").unwrap();
    sw = sw.replace("{site}", &*SITE);

    res.headers_mut().set(ContentType(Mime(TopLevel::Text, SubLevel::Javascript, Vec::new())));
    res.send(sw.as_bytes()).unwrap();
}

fn send_response(mut up_res: hyper::client::Response, mut res: Response) {
    *res.headers_mut() = up_res.headers.clone();
    cleanup_response_headers(res.headers_mut());
    *res.status_mut() = up_res.status.clone();

    let mut res = res.start().unwrap();

    if is_html(&up_res.headers) {
        res.write(format!("<script>navigator.serviceWorker.register('/service-worker.js?site={}');</script>", &*SITE).as_bytes())
            .unwrap();

        let mut buf = String::new();
        println!("recv {:?}", up_res.read_to_string(&mut buf).unwrap());
        //println!("  {:?}", buf);
        buf = buf.replace(&*SITE, "localhost:8081");
        res.write_all(&mut buf.as_bytes()).unwrap();
    } else {
        io::copy(&mut up_res, &mut res).unwrap();
    }

    res.flush().unwrap();
    res.end().unwrap();
}

fn get_upstream_data(mut s_req: &mut Request, uri: String) -> hyper::client::Response {
    let ssl = ::hyper_native_tls::NativeTlsClient::new().unwrap();
    let connector = HttpsConnector::new(ssl);
    let mut client = Client::with_connector(connector);
    //let mut client = Client::new();
    client.set_redirect_policy(RedirectPolicy::FollowNone);
    client.set_read_timeout(Some(::std::time::Duration::new(10, 0)));

    let mut up_req_headers = s_req.headers.clone();
    cleanup_request_headers(&mut up_req_headers);

    //println!("{:#?}", up_req_headers);
    //println!("{:?} {}", s_req.method, uri);

    let mut builder = client.request(s_req.method.clone(), &uri);
    builder = builder.headers(up_req_headers)
        .body(Body::ChunkedBody(&mut s_req));

    //println!("Receiving content");

    builder.send()
        .unwrap()
}

fn cleanup_request_headers(headers: &mut Headers) {
    headers.set(Host {
                    hostname: SITE.clone(),
                    port: Some(80),
                });

    headers.remove::<Referer>();

    // Make sure we only get plain text
    headers.remove_raw("Upgrade");
    headers.remove_raw("Accept-Encoding");
}

fn cleanup_response_headers(headers: &mut Headers) {
    if let Some(Location(location)) = headers.get::<Location>().cloned() {
        println!("redirect: {}", location);
        headers.set(Location(location.replace(&*SITE, "localhost:8081").replace("https://",
                                                                                "http://")));
    }

    // Allow inline service worker insertion script
    headers.remove_raw("Content-Security-Policy");
    headers.remove_raw("X-XSS-Protection");
    headers.remove_raw("Content-Length");
}

