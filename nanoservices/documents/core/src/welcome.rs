use dal::{CreateDocument, UpdateDocumentContent};
use kernel::{Document, NewDocument};
use utils::errors::NanoServiceError;

pub const WELCOME_TITLE: &str = "Welcome to Drafthouse";

pub const WELCOME_CONTENT: &str = r#"# Welcome to Drafthouse

Your collaborative markdown editor. Here's everything you need to get started.

---

## Keyboard Shortcuts

| Shortcut | Action |
|---|---|
| `Cmd/Ctrl+K` | Command palette |
| `Cmd/Ctrl+Shift+\` | Toggle sidebar |
| `Cmd/Ctrl+B` | Bold |
| `Cmd/Ctrl+I` | Italic |
| `Cmd/Ctrl+E` | Inline code |
| `Cmd/Ctrl+Shift+X` | Strikethrough |
| `Cmd/Ctrl+Alt+1` | Heading 1 |
| `Cmd/Ctrl+Alt+2` | Heading 2 |
| `Cmd/Ctrl+Alt+3` | Heading 3 |
| `Cmd/Ctrl+Alt+C` | Code block |
| `Cmd/Ctrl+Shift+7` | Checklist |
| `Cmd/Ctrl+Alt+-` | Horizontal divider |

---

## Markdown Tips

### Text Formatting

**Bold** — `**text**`
*Italic* — `*text*`
~~Strikethrough~~ — `~~text~~`
`Inline code` — `` `code` ``

### Headings

```
# Heading 1
## Heading 2
### Heading 3
```

### Lists

```
- Item one
- Item two
  - Nested item

1. First
2. Second
```

### Links and Images

```
[Link text](https://example.com)
![Alt text](https://example.com/image.png)
```

### Code Blocks

````
```rust
fn main() {
    println!("Hello, world!");
}
```
````

---

## Features

- **Real-time collaboration** — Multiple editors on the same document. Changes sync instantly.
- **Offline editing** — Works without a connection. Changes sync when you reconnect.
- **Preview mode** — Toggle between edit and rendered markdown from the toolbar.
- **Sharing** — Invite collaborators via link with editor or viewer roles.
- **Public documents** — Make any document publicly readable.

---

Feel free to delete this document once you're comfortable. Happy writing!
"#;

pub async fn create_welcome_document<D>(
    dal: &D,
    owner_id: uuid::Uuid,
) -> Result<Document, NanoServiceError>
where
    D: CreateDocument + UpdateDocumentContent,
{
    let doc = dal
        .create_document(NewDocument {
            owner_id,
            title: WELCOME_TITLE.to_string(),
        })
        .await?;

    dal.update_document_content(doc.id, WELCOME_CONTENT.to_string())
        .await?;

    Ok(doc)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use kernel::Document;
    use std::sync::{Arc, Mutex};
    use uuid::Uuid;

    struct MockDal {
        documents: Arc<Mutex<Vec<Document>>>,
        content: Arc<Mutex<std::collections::HashMap<Uuid, String>>>,
        next_id: Uuid,
    }

    impl MockDal {
        fn new() -> Self {
            Self {
                documents: Arc::new(Mutex::new(vec![])),
                content: Arc::new(Mutex::new(std::collections::HashMap::new())),
                next_id: Uuid::new_v4(),
            }
        }
    }

    impl CreateDocument for MockDal {
        fn create_document(
            &self,
            new_doc: kernel::NewDocument,
        ) -> impl std::future::Future<Output = Result<Document, NanoServiceError>> + Send {
            let docs = Arc::clone(&self.documents);
            let id = self.next_id;
            async move {
                let doc = Document {
                    id,
                    owner_id: new_doc.owner_id,
                    title: new_doc.title,
                    is_public: false,
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                };
                docs.lock().unwrap().push(doc.clone());
                Ok(doc)
            }
        }
    }

    impl UpdateDocumentContent for MockDal {
        fn update_document_content(
            &self,
            id: Uuid,
            content: String,
        ) -> impl std::future::Future<Output = Result<(), NanoServiceError>> + Send {
            let map = Arc::clone(&self.content);
            async move {
                map.lock().unwrap().insert(id, content);
                Ok(())
            }
        }
    }

    #[tokio::test]
    async fn create_welcome_document_uses_correct_title() {
        let dal = MockDal::new();
        let owner_id = Uuid::new_v4();
        let doc = create_welcome_document(&dal, owner_id).await.unwrap();
        assert_eq!(doc.title, WELCOME_TITLE);
        assert_eq!(doc.owner_id, owner_id);
    }

    #[tokio::test]
    async fn create_welcome_document_stores_content() {
        let dal = MockDal::new();
        let owner_id = Uuid::new_v4();
        let doc = create_welcome_document(&dal, owner_id).await.unwrap();
        let stored = dal.content.lock().unwrap().get(&doc.id).cloned();
        assert!(stored.is_some());
        let content = stored.unwrap();
        assert!(content.contains("Welcome to Drafthouse"));
        assert!(content.contains("Keyboard Shortcuts"));
        assert!(content.contains("Markdown Tips"));
    }

    #[tokio::test]
    async fn create_welcome_document_returns_correct_doc_id() {
        let dal = MockDal::new();
        let expected_id = dal.next_id;
        let doc = create_welcome_document(&dal, Uuid::new_v4()).await.unwrap();
        assert_eq!(doc.id, expected_id);
    }
}
