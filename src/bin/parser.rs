use std::fs;
use std::collections::HashMap;
use std::cell::RefCell;
use std::rc::{Rc, Weak};
use std::env;
use std::path::PathBuf;
use regex::Regex;


type NodeRef = Rc<RefCell<Node>>;
type WeakNodeRef = Weak<RefCell<Node>>;
type Context = HashMap<String, String>;

fn inject_context(target_str: &str, ctx: &Context) -> String {
    let re = Regex::new(r"\{\{\s*(.+?)\s*\}\}").unwrap();
    let default = String::from("CTX MISS");

    // Replace each placeholder with the corresponding value from the HashMap
    let result = re.replace_all(target_str, |caps: &regex::Captures| {
        let key = caps.get(1).unwrap().as_str();
        ctx.get(key).unwrap_or(&default)
    });
    result.to_string()
}

#[derive(Clone)]
struct Node {
    parent: Option<WeakNodeRef>,
    children: Vec<NodeRef>,
    tag: Option<String>,
    attrs: Option<String>,
    content: Option<String>,
}


#[derive(Debug)]
enum State {
    Attr,
    Content,
    Comment,
    Blank,
    Tag,
    TagOpen,
    TagClose,
}

struct Parser {
    state: State,
    buf: String,
    comment_buf: String,
    attr_buf: String,
    current_node: NodeRef,
    root: NodeRef,
}

impl Node {
    fn new_root() -> Node {
        Node {
            parent: None,
            children: Vec::new(),
            tag: None,
            content: None,
            attrs: None,
        }
    }

    fn add_child(&mut self, tag: Option<String>, attrs: Option<String>, parent: &NodeRef) -> NodeRef {
        let parent = Rc::downgrade(parent);
        let child = Node {
            parent: Some(parent),
            children: Vec::new(),
            tag,
            content: None,
            attrs
        };
        
        // add the child to self.children
        let child_ref = Rc::new(RefCell::new(child));
        self.children.push(Rc::clone(&child_ref));

        // return reference to child
        child_ref
    }

    fn to_html(&self, mut html: String, depth: i32, ctx: &Context) -> String {
        let mut indentation = (0..depth).map(|_| "  ").collect::<String>();
        let attrs_str = if let Some(attrs) = &self.attrs {
            format!(" {}", inject_context(attrs, ctx))
        } else {
            String::from("")
        };
        if let Some(tag) = &self.tag {
            html.push_str(&format!("{}<{}{}>", indentation, tag, attrs_str));
        }
        if let Some(content) = &self.content {
            let content = inject_context(content, ctx);
            html.push_str(&content);
            indentation = String::new();
        } else {
            html.push('\n');
        }

        for child in self.children.iter() {
            html = child.borrow().to_html(html, depth + 1, ctx);
        }

        if let Some(tag) = &self.tag {
            html.push_str(&format!("{}</{}>\n", indentation, tag));
        }
        html
    }

    #[allow(dead_code)]
    fn traverse_dfs(&self, depth: i32) {
        if let Some(tag) = &self.tag {
            let indentation = (0..depth).map(|_| "\t").collect::<String>();
            println!("{}{}", indentation, tag);
        }
        for child in self.children.iter() {
            child.borrow().traverse_dfs(depth + 1);
        }
    }
}

impl Parser {
    fn new() -> Parser {
        let root = Rc::new(RefCell::new(Node::new_root()));
        Parser {
            state: State::Blank,
            buf: String::new(),
            comment_buf: String::new(),
            attr_buf: String::new(),
            current_node: Rc::clone(&root),
            root: Rc::clone(&root),
        }
    }

    fn add_child_to_current_node(&mut self, tag: Option<String>, attrs: Option<String>) {
        let child = self.current_node.borrow_mut().add_child(tag, attrs, &self.current_node);
        self.current_node = child;
    }

    fn is_content(ch: char) -> bool {
        !"<>\n\t\r\\{} ".contains(ch)
    }

    #[allow(dead_code)]
    fn debug_fsm(&self, ch: char) {
        let buf_ref = &self.buf;
        let mut parent_tag = String::new();
        {
            if let Some(parent_weak) = self.current_node.borrow().parent.clone() {
                if let Some(parent) = parent_weak.upgrade() {
                    let parent_borrow = parent.borrow();
                    if let Some(tag) = &parent_borrow.tag {
                        parent_tag = tag.clone();
                    }
                }
            }
        }
        println!("buf: {} State: {:?}, current_node: {:?}, parent: {}, Char: {}", 
                 buf_ref, self.state, self.current_node.borrow().tag, parent_tag, ch);
    }

    fn parse_ch(&mut self, ch: char) {
        // self.debug_fsm(ch);
        match (&self.state, ch) {
            (State::Blank, '<') => {
                self.state = State::Tag;
                self.buf.clear();
            },
            (State::Blank, ch) if Self::is_content(ch) => {
                self.state = State::Content;
                self.buf.push(ch);
            },
            (State::Tag, '/') => {
                self.state = State::TagClose;
            },
            (State::Tag, '!') => {
                self.state = State::Comment;
            },
            (State::Tag, ch) if ch.is_alphanumeric() => {
                self.state = State::TagOpen;
                self.buf.push(ch);
            },
            (State::Comment, '>') => {
                if self.comment_buf.ends_with("--") {
                    self.comment_buf.clear();
                    self.state = State::Blank;
                }
            },
            (State::Comment, _) => {
                self.comment_buf.push(ch);
            },
            (State::TagClose, ch) if ch != '>' => { },
            (State::TagClose, '>') => {
                let parent_weak = self.current_node.borrow().parent.clone();
                if let Some(parent_weak) = parent_weak {
                    if let Some(parent) = parent_weak.upgrade() {
                        self.current_node = parent;
                    } else {
                        // Handle the error case where the parent has already been dropped.
                    }
                }
                self.state = State::Blank;
            },
            (State::TagOpen, ch) if ch == ' ' => {
                self.state = State::Attr;
            },
            (State::TagOpen, ch) if ch.is_alphanumeric() => {
                self.buf.push(ch);
            },
            (State::TagOpen | State::Attr, '>') => {
                let attrs = if !self.attr_buf.is_empty() {
                    Some(self.attr_buf.clone())
                } else { None };
                let tag = Some(self.buf.clone());
                // let child = self.current_node.borrow_mut().add_child(tag);
                self.add_child_to_current_node(tag, attrs);

                self.state = State::Blank;
                self.buf.clear();
                self.attr_buf.clear();
            },
            (State::Attr, ch) => {
                self.attr_buf.push(ch);
            },
            (State::Content, ch) if ch != '<' => {
                self.buf.push(ch);
            },
            (State::Content, '<') => {
                if !self.buf.ends_with('\\') {
                    let content = Some(self.buf.clone());
                    self.current_node.borrow_mut().content = content;
                    self.buf.clear();
                    self.state = State::TagClose;
                }
            },
            _ => {
                // Error or other states
            }
        };
    }

    fn to_html(&self, ctx: &Context) -> String {
        self.root.borrow().to_html(String::new(), -1, ctx)
    }
}

fn get_current_working_dir() -> std::io::Result<PathBuf> {
    env::current_dir()
}

fn parse_file(file_name: &str, ctx: &Context) -> String {
    /* 
    if let Ok(cwd) = get_current_working_dir() {
        println!("Current working dir: {:?}", cwd);
    }
    */
    
    let f = fs::read_to_string(file_name).unwrap();
    let mut parser = Parser::new();
    for ch in f.chars() {
        parser.parse_ch(ch);
    }
    // let _ = parser.root.borrow().traverse_dfs(0);
     
    parser.to_html(ctx)
}


fn main() {
    let filename = "./templates/test.html";
    let mut ctx = Context::new();
    ctx.insert("variable".into(), "1234".into());
    let h = parse_file(filename, &ctx);

    println!("{}", h);
     
}





