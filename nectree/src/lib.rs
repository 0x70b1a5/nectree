use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uqbar_process_lib::{
    await_message, get_payload, get_state,
    http::{
        bind_http_path, bind_http_static_path, send_response, serve_index_html, HttpServerRequest,
        IncomingHttpRequest, StatusCode,
    },
    println, set_state,
    vfs::{open_file, File},
    Address, Message,
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

type LinkTree = HashMap<String, Link>;

fn handle_http_server_request(
    our: &Address,
    ipc: &[u8],
    link_tree: &mut LinkTree,
    html_file: &mut File,
) -> anyhow::Result<()> {
    let Ok(server_request) = serde_json::from_slice::<HttpServerRequest>(ipc) else {
        println!("nectree: couldn't parse request!");
        return Ok(());
    };

    match server_request {
        HttpServerRequest::Http(IncomingHttpRequest { method, .. }) => {
            match method.as_str() {
                "POST" => {
                    let Some(payload) = get_payload() else {
                        println!("no payload in nectree POST request");
                        // TODO, send http_responses too?
                        return Ok(());
                    };

                    println!("Payload: {:?}", String::from_utf8(payload.bytes.clone()));

                    // note here. adding a single js script into the html file will enable both json post requests,
                    // and dynamic reloading.
                    let Ok(link_request) = serde_json::from_slice::<LinkRequest>(&payload.bytes) else {
                        println!("nectree: couldn't parse link request!");
                        return Ok(());
                    };

                    match link_request {
                        LinkRequest::Save(link) => {
                            link_tree.insert(link.name.clone(), link);

                            save_and_render_html(our, link_tree, html_file)?;
                        }
                        LinkRequest::Delete { name } => {
                            link_tree.remove(&name);
                            save_and_render_html(our, link_tree, html_file)?;
                        }
                    }
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

fn save_and_render_html(
    our: &Address,
    link_tree: &LinkTree,
    html_file: &mut File,
) -> anyhow::Result<()> {
    // Save the LinkTree to state
    let state = serde_json::to_vec(link_tree)?;
    set_state(&state);

    let mut links: Vec<&Link> = link_tree.values().collect();
    links.sort_by_key(|link| link.order);

    let html_links: String = links
        .iter()
        .map(|link| {
            format!(
                "<li><a target=\"_blank\" href=\"{}\"><img src=\"{}\" alt=\"{}\" style=\"max-width:100px; max-height:100px;\">{}</a><p>{}</p><button class=\"delete-button\" data-name=\"{}\">X</button></li>",
                link.url, link.image, link.description, link.name, link.description, link.name
            )
        })
        .collect();

    let html_body = format!("<ul>\n{}\n</ul>", html_links);
    let html_header = generate_html_header();
    let html = format!("{}{}\n</body>\n</html>", html_header, html_body);

    // Write the HTML string to the file
    html_file.write(html.as_bytes())?;
    // might not be necessary
    html_file.sync_all()?;

    bind_http_static_path("/", false, false, Some("text/html".into()), html.into_bytes())?;

    Ok(())
}

fn generate_html_header() -> String {
    let html_header = r#"
    <!DOCTYPE html>
    <html>
    <head>
        <title>NecTree</title>
        <style>
            body {
                font-family: Arial, sans-serif;
            }
            h2 {
                color: #333;
            }
        </style>
    </head>
    <body>
        <h2>NecTree</h2>
        <form id="linkForm">
            <label for="name">Name:</label><br>
            <input type="text" id="name" name="name"><br>
            <label for="url">URL:</label><br>
            <input type="text" id="url" name="url"><br>
            <label for="image">Image URL:</label><br>
            <input type="text" id="image" name="image"><br>
            <label for="description">Description:</label><br>
            <input type="text" id="description" name="description"><br>
            <label for="order">Order:</label><br>
            <input type="number" id="order" name="order"><br>
            <input type="submit" value="Add Link">
        </form>
    
        <script>
            document.getElementById('linkForm').addEventListener('submit', function(event) {
                event.preventDefault();
    
                var link = {
                    name: document.getElementById('name').value,
                    url: document.getElementById('url').value,
                    image: document.getElementById('image').value,
                    description: document.getElementById('description').value,
                    order: Number(document.getElementById('order').value)
                };
    
                fetch('post', {
                    method: 'POST',
                    headers: {
                        'Content-Type': 'application/json'
                    },
                    body: JSON.stringify({ Save: link })
                }).then(function() {
                    location.reload();
                });
            });

            function addDeleteButtonEventListeners() {
                var deleteButtons = document.getElementsByClassName('delete-button');
                for (var i = 0; i < deleteButtons.length; i++) {
                    deleteButtons[i].addEventListener('click', function(event) {
                        event.preventDefault();

                        var name = event.target.getAttribute('data-name');

                        fetch('post', {
                            method: 'POST',
                            headers: {
                                'Content-Type': 'application/json'
                            },
                            body: JSON.stringify({ Delete: { name: name } })
                        }).then(function() {
                            location.reload();
                        });
                    });
                }
            }

            addDeleteButtonEventListeners();
        </script>
    "#;
    html_header.to_string()
}

fn handle_message(
    our: &Address,
    mut link_tree: &mut LinkTree,
    mut html_file: &mut File,
) -> anyhow::Result<()> {
    let message = await_message()?;

    match message {
        Message::Response { .. } => {
            println!("nnotes: got response - {:?}", message);
            return Ok(());
        }
        Message::Request { ref ipc, .. } => {
            // Requests that come from our http server, handle intranode later too.
            handle_http_server_request(our, ipc, &mut link_tree, &mut html_file)?;
        }
    }

    Ok(())
}

// can make stateless, you'll deal with the html parsing sir.
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

        // Get previous LinkTree or create a new one
        let mut link_tree: LinkTree = match get_state() {
            Some(state) => serde_json::from_slice::<LinkTree>(&state).unwrap(),
            None => HashMap::new(),
        };

        // open existing html file, read it's bytes.
        let mut html_file =
            open_file(&format!("{}/pkg/ui/index.html", our.package_id()), false).unwrap();

        let html = html_file.read().unwrap();

        // serve the "static" index.html file
        // quick note, we can't use bind_path and bind static path together.

        // bind the "/" path for post requests.
        bind_http_path("/post", false, false).unwrap();
        bind_http_static_path("/", false, false, Some("text/html".into()), html).unwrap();

        loop {
            match handle_message(&our, &mut link_tree, &mut html_file) {
                Ok(()) => {}
                Err(e) => {
                    println!("nectree: error: {:?}", e);
                }
            };
        }
    }
}
