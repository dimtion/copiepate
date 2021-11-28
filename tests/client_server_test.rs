use std::{
    error::Error,
    sync::{Arc, RwLock},
    thread,
    time::Duration,
};

use clipboard::ClipboardProvider;

const ADDRESS: &str = "127.0.0.1:2423";

struct TestClipboardContext {
    pub clipboard_content: Arc<RwLock<String>>,
}

impl ClipboardProvider for TestClipboardContext {
    fn new() -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            clipboard_content: Arc::new(RwLock::new(String::new())),
        })
    }

    fn get_contents(&mut self) -> Result<String, Box<dyn Error>> {
        let lock = self.clipboard_content.read().unwrap();
        Ok(lock.to_string())
    }

    fn set_contents(&mut self, content: String) -> Result<(), Box<dyn Error>> {
        let lock = &mut self.clipboard_content.write().unwrap();
        **lock = content;
        Ok(())
    }
}

#[test]
fn test_happy_path() -> Result<(), Box<dyn Error>> {
    let test_message = "Test Message";
    let clipboard_content = Arc::new(RwLock::new(String::new()));
    let mut clipboard_ctx = TestClipboardContext {
        clipboard_content: clipboard_content.clone(),
    };

    // 1. Start server
    thread::spawn(move || {
        let mut clipboard_ctx = TestClipboardContext {
            clipboard_content: clipboard_content.clone(),
        };
        let mut server = copiepate::server::Server::<TestClipboardContext> {
            address: ADDRESS,
            clipboard_ctx: &mut clipboard_ctx,
        };
        server.start().unwrap();
    });

    thread::sleep(Duration::from_millis(100));

    // 2. Send clipboard
    let client = copiepate::client::Client { address: ADDRESS };
    client.send_message(test_message.as_bytes())?;

    // 3. Wait
    thread::sleep(Duration::from_millis(100));

    // 4. Check clipboard
    assert_eq!(test_message, clipboard_ctx.get_contents()?);

    Ok(())
}
