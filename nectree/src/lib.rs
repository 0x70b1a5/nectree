use serde::{Serialize, Deserialize};

use uqbar_process_lib::{await_message, print_to_terminal, Address, Message, ProcessId, Request, Response};

wit_bindgen::generate!({
    path: "wit",
    world: "process",
    exports: {
        world: Component,
    },
});

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Link {
    id: String,
    display_name: String,
    target: String,
    image: String,
    description: String,
    order: u32,
}

#[derive(Debug, Serialize, Deserialize)]
enum LinkRequest {
    Get,
    Create(Link),
    Edit(Link),
    Delete(String),
}

#[derive(Debug, Serialize, Deserialize)]
enum LinkResponse {
    Ack,
    Get {
        link_tree: LinkTree,
    },
}

type LinkTree = Vec<Link>;

fn handle_message (
    our: &Address,
    link_tree: &mut LinkTree,
) -> anyhow::Result<()> {
    let message = await_message().unwrap();

    match message {
        Message::Response { .. } => {
            print_to_terminal(0, &format!("nectree: unexpected Response: {:?}", message));
            panic!("");
        },
        Message::Request { ref source, ref ipc, .. } => {
            if source != our {
                print_to_terminal(0, &format!("nectree: unexpected Request: {:?}", message));
                panic!("");
            }
            match serde_json::from_slice(ipc)? {
                LinkRequest::Create(link) => {
                    link_tree.push(link);
                    Response::new()
                        .ipc(serde_json::to_vec(&LinkResponse::Ack).unwrap())
                        .send()
                        .unwrap();
                },
                LinkRequest::Edit(link) => {
                    let mut found = false;
                    for found_link in link_tree.iter_mut() {
                        if found_link.id == link.id {
                            *found_link = link;
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        return Err(anyhow::anyhow!("link not found"));
                    }
                    Response::new()
                        .ipc(serde_json::to_vec(&LinkResponse::Ack).unwrap())
                        .send()
                        .unwrap();
                },
                LinkRequest::Delete(id) => {
                    let mut found = false;
                    for i in 0..link_tree.len() {
                        if link_tree[i].id == id {
                            link_tree.remove(i);
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        return Err(anyhow::anyhow!("link not found"));
                    }
                    Response::new()
                        .ipc(serde_json::to_vec(&LinkResponse::Ack).unwrap())
                        .send()
                        .unwrap();
                },
                LinkRequest::Get => {
                    Response::new()
                        .ipc(serde_json::to_vec(&LinkResponse::Get {
                            link_tree: link_tree.clone(),
                        }).unwrap())
                        .send()
                        .unwrap();
                },
            }
        },
    }
    Ok(())
}

struct Component;
impl Guest for Component {
    fn init(our: String) {
        print_to_terminal(0, "nectree: begin");

        let our = Address::from_str(&our).unwrap();
        let mut link_tree: LinkTree = Vec::new();

        loop {
            match handle_message(&our, &mut link_tree) {
                Ok(()) => {},
                Err(e) => {
                    print_to_terminal(0, format!(
                        "nectree: error: {:?}",
                        e,
                    ).as_str());
                },
            };
        }
    }
}
