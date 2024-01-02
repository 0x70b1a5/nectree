use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use uqbar_process_lib::{
    await_message, println, Address, Message, ProcessId, Request, Response, get_payload,
    http::{IncomingHttpRequest, HttpServerRequest, StatusCode, send_response, serve_index_html, bind_http_static_path, bind_http_path},
    vfs::open_file,
};

wit_bindgen::generate!({
    path: "wit",
    world: "process",
    exports: {
        world: Component,
    },
});

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Link {
    name: String,
    url: String,
    image: String,
    description: String,
    order: u32,
}

#[derive(Debug, Serialize, Deserialize)]
enum LinkRequest {
    Save(Link),
    Delete { name: String },
}

#[derive(Debug, Serialize, Deserialize)]
enum LinkResponse {
    Get { link_tree: LinkTree },
}

type LinkTree = HashMap<String, Link>;

fn handle_http_server_request (
    our: &Address,
    ipc: &[u8],
    mut link_tree: &mut LinkTree,
) -> anyhow::Result<()> {
    let Ok(server_request) = serde_json::from_slice::<HttpServerRequest>(ipc) else {
        println!("nectree: couldn't parse request!");
        return Ok(());
    };

    match server_request {
        HttpServerRequest::Http(IncomingHttpRequest { method, .. }) => {
            match method.as_str() {
                // Get a path
                "GET" => {
                    let mut headers = HashMap::new();
                    headers.insert("Content-Type".to_string(), "application/json".to_string());
                    
                    // Send an http response via the http server
                }
                // Send a message
                "POST" => {
                    let Some(payload) = get_payload() else {
                        println!("no payload in nectree POST request");
                        return Ok(());
                    };

                    // Send an http response via the http server
                    send_response(StatusCode::CREATED, None, vec![])?;
                }
                _ => {
                    // Method not allowed
                    send_response(StatusCode::METHOD_NOT_ALLOWED, None, vec![])?;
                }
            }
        }
        _ => {
            // Method not allowed
            send_response(StatusCode::METHOD_NOT_ALLOWED, None, vec![])?;
        }
    };

Ok(())
    
}

fn handle_message(
    our: &Address,
    mut link_tree: &mut LinkTree,
) -> anyhow::Result<()> {
    let message = await_message()?;

    match message {
        Message::Response { .. } => {
            println!("nnotes: got response - {:?}", message);
            return Ok(());
        }
        Message::Request {
            ref ipc,
            ..
        } => {
            // Requests that come from our http server, handle intranode later too. 
            handle_http_server_request(our, ipc, &mut link_tree)?;

        }
    }

    Ok(())
}

/// 1. read in html/markdown file 
/// 2. parse it into a tree

/// 1. make linktree into an html/markdown file
/// 2. save_file
/// 3. bind_path again.

struct Component;
impl Guest for Component {
    fn init(our: String) {
        println!("nectree: begin");

        let our = Address::from_str(&our).unwrap();
        let mut link_tree: LinkTree = HashMap::new();

        // Bind the path, and serve the index.html file
        serve_index_html(&our, "ui").unwrap();
        bind_http_path("/", false, false).unwrap();

        loop {
            match handle_message(&our, &mut link_tree) {
                Ok(()) => {},
                Err(e) => {
                    println!("nectree: error: {:?}", e);
                },
            };
        }
    }
}
