use serde::{Deserialize, Serialize};
use std::{collections::HashMap, str::FromStr};
use nectar_process_lib::{
    await_message, get_blob, get_state, call_init,
    http::{
        bind_http_path, bind_http_static_path, send_response, HttpServerRequest,
        IncomingHttpRequest, StatusCode,
    },
    println, set_state,
    vfs::{open_file, File},
    Address, Message,
};

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

const HTML_TEMPLATE: &str = include_str!("../../pkg/ui/template.html");

fn handle_http_server_request(
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
                    let Some(payload) = get_blob() else {
                        println!("no payload in nectree POST request");
                        return Ok(());
                    };

                    let Ok(link_request) = serde_json::from_slice::<LinkRequest>(&payload.bytes) else {
                        println!("nectree: couldn't parse link request!");
                        return Ok(());
                    };

                    match link_request {
                        LinkRequest::Save(link) => {
                            link_tree.insert(link.name.clone(), link);
                            save_and_render_html(link_tree, html_file)?;
                        }
                        LinkRequest::Delete { name } => {
                            link_tree.remove(&name);
                            save_and_render_html(link_tree, html_file)?;
                        }
                    }
                    send_response(StatusCode::CREATED, None, vec![])?;
                }
                "GET" => {
                    let html = html_file.read()?;
                    let mut headers: HashMap<String, String> = HashMap::new();
                    headers.insert("Content-Type".into(), "text/html".into());
                    send_response(StatusCode::OK, Some(headers), html)?;
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

fn save_and_render_html(link_tree: &LinkTree, html_file: &mut File) -> anyhow::Result<()> {
    // Save the LinkTree to state
    // println!("in save and render");
    let state = serde_json::to_vec(link_tree)?;
    set_state(&state);

    let mut links: Vec<&Link> = link_tree.values().collect();
    let html: String;
    if links.len() == 0 {
        // println!("length 0, pre and post html: {}", HTML_TEMPLATE);
        html = HTML_TEMPLATE.replace("linksgohere", &"No links yet.");
        // println!("html post len 0 replace: {}", html);
    } else {
        links.sort_by_key(|link| link.order);

        let html_links: String = links
            .iter()
            .map(|link| {
                format!(
                    r#"

<a href="{}" target="_blank" class="rounded-md bg-gray-200 p-2 mb-2 flex items-center hover:bg-gray-300">
    <img class="w-16 h-16 bg-gray-400 rounded-md mr-2" src={}/>
    <div>
        <h3 class="text-lg font-semibold">{}</h3>
        <p class="text-sm">{}</p>
    </div>
    <button class="ml-auto bg-red-500 text-white px-2 py-1 rounded-md" onclick="deleteLink('{}')">Delete</button>
</a>
                    "#,
                    link.url, link.image, link.name, link.description, link.name
                )
            })
            .collect();
        // println!("html pre len not 0 replace: {}", HTML_TEMPLATE);
        html = HTML_TEMPLATE.replace("linksgohere", &html_links);
        // println!("POST PROBLME {}", html);
    }

    // Write the HTML string to the file
    html_file.write(html.as_bytes())?;
    html_file.sync_all()?;

    Ok(())
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
        Message::Request { ref body, .. } => {
            // Requests that come from our http server, handle intranode later too.
            handle_http_server_request(body, &mut link_tree, &mut html_file)?;
        }
    }

    Ok(())
}

wit_bindgen::generate!({
    path: "wit",
    world: "process",
    exports: {
        world: Component,
    },
});

call_init!(init);

fn init(our: Address) {
    println!("nectree: begin");

    // Get previous LinkTree or create a new one
    let mut link_tree: LinkTree = match get_state() {
        Some(state) => serde_json::from_slice::<LinkTree>(&state).unwrap(),
        None => HashMap::new(),
    };

    // open existing html file, read it's bytes.
    let mut html_file =
        open_file(&format!("{}/pkg/ui/index.html", our.package_id()), false).unwrap();

    let favicon =
        open_file(&format!("{}/pkg/ui/favicon.ico", our.package_id()), false).unwrap();

    bind_http_path("/post", false, false).unwrap();
    bind_http_static_path(
        "/favicon.ico",
        false,
        false,
        Some("image/x-icon".into()),
        favicon.read().unwrap(),
    )
    .unwrap();
    bind_http_path("/", false, false).unwrap();

    loop {
        match handle_message(&our, &mut link_tree, &mut html_file) {
            Ok(()) => {}
            Err(e) => {
                println!("nectree: error: {:?}", e);
            }
        };
    }
}