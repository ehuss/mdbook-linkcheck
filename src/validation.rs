use std::path::Path;
use std::ffi::OsStr;
use failure::Error;
use mdbook::renderer::RenderContext;
use url::Url;
use reqwest;

use errors::{EmptyLink, FileNotFound, MdSuggestion, UnsuccessfulStatus};
use {Config, Link};

pub fn check_link(link: &Link, ctx: &RenderContext, cfg: &Config) -> Result<(), Error> {
    trace!("Checking {}", link);

    if link.url.is_empty() {
        let err = EmptyLink::new(&link.chapter.path, link.line_number());
        return Err(Error::from(err));
    }

    match Url::parse(&link.url) {
        Ok(link_url) => validate_external_link(link_url, cfg),
        Err(_) => check_link_in_book(link, ctx),
    }
}

fn validate_external_link(url: Url, cfg: &Config) -> Result<(), Error> {
    if cfg.follow_web_links {
        debug!("Fetching \"{}\"", url);

        let response = reqwest::get(url.clone())?;
        let status = response.status();

        if status.is_success() {
            Ok(())
        } else {
            trace!("Unsuccessful Status {} for {}", status, url);
            Err(Error::from(UnsuccessfulStatus(status)))
        }
    } else {
        debug!("Ignoring \"{}\"", url);
        Ok(())
    }
}

fn check_link_in_book(link: &Link, ctx: &RenderContext) -> Result<(), Error> {
    let path = Path::new(&link.url);

    let extension = path.extension();
    if extension == Some(OsStr::new("md")) {
        // linking to a `*.md` file is an error because we don't (yet)
        // automatically translate these links into `*.html`.
        let err = MdSuggestion::new(path, &link.chapter.path, link.line_number());
        Err(Error::from(err))
    } else if extension == Some(OsStr::new("html")) {
        check_link_to_chapter(link, ctx)
    } else {
        check_asset_link_is_valid(link, ctx)
    }
}

fn check_link_to_chapter(link: &Link, ctx: &RenderContext) -> Result<(), Error> {
    let path = match link.url.find("#") {
        Some(ix) => &link.url[..ix],
        None => &link.url,
    };

    let src = ctx.root.join(&ctx.config.book.src);

    // note: all chapter links are relative to the `src/` directory
    let chapter_path = src.join(path).with_extension("md");
    debug!("Searching for {}", chapter_path.display());

    if chapter_path.exists() {
        Ok(())
    } else {
        Err(Error::from(FileNotFound::new(
            path,
            &link.chapter.path,
            link.line_number(),
        )))
    }
}

/// Check the link is to a valid asset inside the book's `src/` directory. The
/// HTML renderer will copy this to the destination directory accordingly.
fn check_asset_link_is_valid(link: &Link, ctx: &RenderContext) -> Result<(), Error> {
    let path = Path::new(&link.url);
    let src = ctx.root.join(&ctx.config.book.src);

    debug_assert!(
        src.is_absolute(),
        "mdbook didn't give us the book root's absolute path"
    );

    let full_path = if path.is_absolute() {
        src.join(&path)
    } else {
        let directory = match link.chapter.path.parent() {
            Some(parent) => src.join(parent),
            None => src.clone(),
        };

        directory.join(&path)
    };

    // by this point we've resolved the link relative to the source chapter's
    // directory, and turned it into an absolute path. This *should* match a
    // file on disk.
    debug!("Searching for {}", full_path.display());

    match full_path.canonicalize() {
        Err(_) => Err(Error::from(FileNotFound::new(
            path,
            &link.chapter.path,
            link.line_number(),
        ))),
        Ok(p) => if p.exists() {
            Ok(())
        } else {
            Err(Error::from(FileNotFound::new(
                p,
                &link.chapter.path,
                link.line_number(),
            )))
        },
    }
}