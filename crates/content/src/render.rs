use pulldown_cmark::{Options, Parser, html::push_html};

/// Modrinth project bodies are markdown with inline HTML
pub fn markdown(md: &str) -> String {
  let mut out = String::new();
  push_html(
    &mut out,
    Parser::new_ext(
      md,
      Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS,
    ),
  );
  html(&out)
}

/// CurseForge descriptions are HTML
pub fn html(html: &str) -> String {
  ammonia::clean(html)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn markdown_renders_and_strips_scripts() {
    let out = markdown("# Title\n\n**bold** <script>alert(1)</script><img src=x onerror=alert(1)>");
    assert!(out.contains("<h1>Title</h1>"));
    assert!(out.contains("<strong>bold</strong>"));
    assert!(!out.contains("<script"));
    assert!(!out.contains("onerror"));
  }

  #[test]
  fn html_strips_scripts() {
    let out = html(r#"<p onclick="x()">hi</p><script>alert(1)</script>"#);
    assert_eq!(out, "<p>hi</p>");
  }
}
