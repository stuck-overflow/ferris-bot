pub struct QueueManager {
    queue: Vec<String>,
}

pub enum UserType {
    Default,
    Subscriber,
}

impl QueueManager {
    pub fn new() -> QueueManager {
        QueueManager { queue: Vec::new() }
    }
    pub fn join(&mut self, name: String, user_type: UserType) -> Result<(), ()> {
        self.queue.push(name);
        Ok(())
    }
    pub fn queue(&self) -> &[String] {
        self.queue.as_slice()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use random_string::{Charset, Charsets, RandomString};
    fn gen_random_user() -> String {
        let charset = Charset::from_charsets(Charsets::Numbers);
        let data = RandomString::generate(6, &charset);
        data.to_string()
    }

    #[test]
    fn test_queue() {
        let mut users = vec![];
        let mut queue_man = QueueManager::new();
        for _ in 0..3 {
            let random_user = gen_random_user();
            assert!(queue_man
                .join(random_user.clone(), UserType::Default)
                .is_ok());
            users.push(random_user);
        }
        assert_eq!(queue_man.queue(), users.as_slice());
    }
}
