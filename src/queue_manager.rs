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
    use rand::{thread_rng, Rng, distributions::Alphanumeric};
    fn gen_random_user() -> String {
        let rng = thread_rng();

        rng.sample_iter(Alphanumeric)
            .take(6)
            .map(char::from)
            .collect()
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
