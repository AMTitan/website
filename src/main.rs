use chrono::NaiveDate;
use handlebars::Handlebars;
use rss::{ChannelBuilder, Guid, Item};
use serde_derive::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use toml::{Table, Value};
use walkdir::WalkDir;

#[derive(Clone, Serialize)]
struct Blog {
    config: Table,
    path: PathBuf,
}

#[derive(Clone, Serialize)]
struct BlogPage {
    title: String,
    blogs: Vec<Blog>,
    before: Option<usize>,
    after: Option<usize>,
}

#[derive(Serialize)]
struct Jsonfeed {
    version: String,
    title: String,
    items: Vec<JsonfeedItems>,
    icon: String,
    home_page_url: String,
    feed_url: String,
}

#[derive(Serialize)]
struct JsonfeedItems {
    id: String,
    url: String,
    title: String,
    content_html: String,
    date_published: String,
}

#[derive(Deserialize)]
struct Config {
    name: String,
    home_page: String,
    icon: String,
}

fn main() {
    // make public folder
    let _ = fs::remove_dir_all("public");
    fs::create_dir("public").expect("could not make the folder \"public\"");

    let config: Config = toml::from_str(
        &fs::read_to_string("config.toml").expect("could not read file config.toml"),
    )
    .expect("Cant convert your config to toml");

    //copy all static
    for e in WalkDir::new("static").into_iter().filter_map(|e| e.ok()) {
        if e.metadata().unwrap().is_file() {
            let mut path = e.path();
            path = path.strip_prefix("static/").unwrap(); // should never fail
            let new_path = Path::new("public").join(path);
            let mut parent = new_path.to_path_buf();
            parent.pop();
            fs::create_dir_all(parent.clone())
                .unwrap_or_else(|_| panic!("Cant make the folders {}", parent.display()));
            fs::copy(e.path(), new_path).unwrap_or_else(|_| {
                panic!(
                    "failed to copy static/{} to public/{}",
                    path.display(),
                    path.display()
                )
            });
        }
    }

    // make template
    let mut reg = Handlebars::new();
    reg.register_template_string(
        "main",
        fs::read_to_string("template.html.hbs").expect("cant read template.html.hbs"),
    )
    .expect("cant make template");

    //format all pages
    for e in WalkDir::new("pages").into_iter().filter_map(|e| e.ok()) {
        if e.metadata().unwrap().is_file() {
            let path = e.path();
            let contents = fs::read_to_string(path)
                .unwrap_or_else(|_| panic!("Cant read file {}", path.display()));
            let mut config = format!(
                "{}\ncontent = \"\"",
                contents
                    .split("+++")
                    .nth(1)
                    .unwrap_or_else(|| panic!("{} does not have a +++ section", path.display()))
            )
            .parse::<Table>()
            .unwrap();
            let content = contents
                .split("+++")
                .nth(2)
                .unwrap_or_else(|| panic!("{} does not have a +++ section", path.display()));
            let config_content = config.get_mut("content").unwrap();
            *config_content = Value::try_from(md_to_html(content)).unwrap();
            let path = Path::new("public")
                .join(path.strip_prefix("pages").unwrap())
                .with_extension("html");
            let mut parent = path.to_path_buf();
            parent.pop();
            fs::create_dir_all(parent.clone())
                .unwrap_or_else(|_| panic!("Cant make the folders {}", parent.display()));
            let mut file = File::create(path.clone())
                .unwrap_or_else(|_| panic!("Cant make file {}", path.display()));
            file.write_all(
                reg.render("main", &config)
                    .unwrap_or_else(|_| panic!("Cant render {}", path.display()))
                    .as_bytes(),
            )
            .unwrap_or_else(|_| panic!("Cant write to file {}", path.display()))
        }
    }

    //do the blogs
    if let Ok(blogs) = fs::read_dir("blogs") {
        fs::create_dir("public/blogs").expect("could not make the folder \"public/blogs\"");
        let mut all_blogs: Vec<Blog> = Vec::new();
        for i in blogs.flatten() {
            let path = i.path();
            let contents = fs::read_to_string(path.clone())
                .unwrap_or_else(|_| panic!("Cant read file {}", path.display()));
            let mut config = format!(
                "{}\ncontent = \"\"",
                contents
                    .split("+++")
                    .nth(1)
                    .unwrap_or_else(|| panic!("{} does not have a +++ section", path.display()))
            )
            .parse::<Table>()
            .unwrap();
            let content = contents
                .split("+++")
                .nth(2)
                .unwrap_or_else(|| panic!("{} does not have a +++ section", path.display()));
            let config_content = config.get_mut("content").unwrap();
            *config_content = Value::try_from(md_to_html(content)).unwrap();
            let path = path.strip_prefix("blogs").unwrap().with_extension("html");
            let mut file = File::create(Path::new("public/blogs").join(path.clone()))
                .unwrap_or_else(|_| panic!("Cant make file public/blogs/{}", path.display()));
            file.write_all(
                reg.render("main", &config)
                    .unwrap_or_else(|_| panic!("Cant render public/blogs/{}", path.display()))
                    .as_bytes(),
            )
            .unwrap_or_else(|_| panic!("Cant write to file public/blogs/{}", path.display()));
            all_blogs.push(Blog {
                config,
                path: path.to_path_buf(),
            });
        }
        all_blogs.sort_by(|a, b| {
            NaiveDate::parse_from_str(
                a.config["date"]
                    .as_str()
                    .unwrap_or_else(|| panic!("{} does not have a date", a.path.display())),
                "%Y-%m-%d",
            )
            .unwrap_or_else(|_| panic!("Cant convert {} date", a.path.display()))
            .cmp(
                &NaiveDate::parse_from_str(
                    b.config["date"]
                        .as_str()
                        .unwrap_or_else(|| panic!("{} does not have a date", b.path.display())),
                    "%Y-%m-%d",
                )
                .unwrap_or_else(|_| panic!("Cant convert {} date", b.path.display())),
            )
        });
        let mut i = 0;
        let blog_pages = all_blogs
            .chunks(10)
            .map(|x| {
                let before = if i > 0 { Some(i - 1) } else { None };
                let after = if i < all_blogs.len() / 10 {
                    Some(i + 1)
                } else {
                    None
                };
                i += 1;
                BlogPage {
                    title: "Blogs".to_string(),
                    blogs: x.to_vec(),
                    before,
                    after,
                }
            })
            .collect::<Vec<_>>();
        let json_blogs = serde_json::to_string(&Jsonfeed {
            version: "https://jsonfeed.org/version/1".to_string(),
            title: format!("{}'s Blog", config.name),
            icon: config.icon,
            home_page_url: config.home_page.clone(),
            feed_url: format!("{}/blogs.json", config.home_page),
            items: blog_pages
                .first()
                .expect("cant get the first blog page")
                .blogs
                .iter()
                .map(|x| JsonfeedItems {
                    id: format!("{}/blogs/{}", config.home_page.clone(), x.path.display()),
                    url: format!("{}/blogs/{}", config.home_page, x.path.display()),
                    title: x.config["title"]
                        .as_str()
                        .expect("cant get the title of the blog")
                        .to_string(),
                    content_html: x.config["content"]
                        .as_str()
                        .expect("cant get the content of a blog (should never happen)")
                        .to_string(),
                    date_published: format!(
                        "{}T00:00:00+00:00",
                        x.config["date"].as_str().expect("no date on blog")
                    ),
                })
                .collect(),
        })
        .expect("Cant write the json blogs");
        let rss_blogs = ChannelBuilder::default()
            .title(format!("{}'s Blog", config.name))
            .link(format!("{}/blogs", config.home_page))
            .items(
                blog_pages
                    .first()
                    .expect("cant get the first blog page")
                    .blogs
                    .iter()
                    .map(|x| {
                        let mut item = Item::default();
                        item.set_guid({
                            let mut guid = Guid::default();
                            guid.set_permalink(true);
                            guid.set_value(format!(
                                "{}/blogs/{}",
                                config.home_page.clone(),
                                x.path.display()
                            ));
                            guid
                        });
                        item.set_title(
                            x.config["title"]
                                .as_str()
                                .expect("Cant get the title of a blog")
                                .to_string(),
                        );
                        item.set_link(format!(
                            "{}/blogs/{}",
                            config.home_page.clone(),
                            x.path.display()
                        ));
                        item.set_description(
                            x.config["content"]
                                .as_str()
                                .expect("cant get the content of a blog (should never happen)")
                                .to_string(),
                        );
                        item.set_pub_date(
                            NaiveDate::parse_from_str(
                                x.config["date"].as_str().expect("no date on blog"),
                                "%Y-%m-%d",
                            )
                            .expect("Cant read date")
                            .format("%a, %d %b %Y 00:00:00 +0000")
                            .to_string(),
                        );
                        item
                    })
                    .collect::<Vec<_>>(),
            )
            .build();

        let mut file = File::create("public/blogs.json").expect("Cant make blogs.json");
        file.write_all(json_blogs.as_bytes())
            .expect("Cant write to blogs.json");
        let mut file = File::create("public/blogs.rss").expect("Cant make blogs.rss");
        file.write_all(rss_blogs.to_string().as_bytes())
            .expect("Cant write to blogs.rss");

        for (x, i) in blog_pages.iter().enumerate() {
            let path_name = format!("public/blogs-{x}.html");
            let path = Path::new(&path_name);
            let mut file =
                File::create(path).unwrap_or_else(|_| panic!("Cant make file {}", path.display()));
            file.write_all(
                reg.render("main", &i)
                    .unwrap_or_else(|_| panic!("Cant render {}", path.display()))
                    .as_bytes(),
            )
            .unwrap_or_else(|_| panic!("Cant write to file {}", path.display()));
        }
    }
}

use emojicons::EmojiFormatter;
use pulldown_cmark::{html, Options, Parser};

pub fn md_to_html(input: &str) -> String {
    let input = EmojiFormatter(input).to_string();
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    let parser = Parser::new_ext(&input, options);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    html_output
}
