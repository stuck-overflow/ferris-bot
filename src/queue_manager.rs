use std::collections::VecDeque;

pub struct QueueManager {
    queue: VecDeque<String>,
    capacity: usize,
}

pub enum UserType {
    Default,
    Subscriber,
}
#[derive(Debug)]
pub enum QueueManagerError {
    QueueFull,
    UserAlreadyInQueue,
}

impl QueueManager {
    pub fn new(capacity: usize) -> QueueManager {
        QueueManager { queue: VecDeque::new(), capacity }
    }
    pub fn join(&mut self, name: String, user_type: UserType) -> Result<(), QueueManagerError> {
        if self.queue.contains(&name) {
            println!("queue contains name already");
            return Err(QueueManagerError::UserAlreadyInQueue);
        }
        if self.queue.len() == self.capacity {
            println!("queue is at capacity");
            return Err(QueueManagerError::QueueFull);
        } 
        self.queue.push_back(name);
        Ok(())
    }
    pub fn queue(&self) -> impl Iterator<Item=&String> {
        self.queue.iter()
    }
    pub fn next(&mut self) -> Option<String> {
        self.queue.pop_front()
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
        let mut queue_man = QueueManager::new(3);
        for _ in 0..3 {
            let random_user = gen_random_user();
            assert!(queue_man
                .join(random_user.clone(), UserType::Default)
                .is_ok());
            // Second invocation with same user should fail.
            let result = queue_man
                .join(random_user.clone(), UserType::Default);
            
            assert!(matches!(result, Err(QueueManagerError::UserAlreadyInQueue)));
            users.push(random_user);
        }
        let random_user = gen_random_user();
        // Queue should have reached capacity by now, so any new user should fail.
        let result = queue_man.join(random_user.clone(), UserType::Default);
        assert!(matches!(result, Err(QueueManagerError::QueueFull)));
       
        // TODO iterate on queue and check entries.
        //assert_eq!(queue_man.queue().collect::<&String>().as_slice(), users.as_slice());

        // call next(), the user should be users.get(0);
        for i in 0..3 {
            assert_eq!(queue_man.next(), Some(users.get(i).unwrap().to_owned()));
        }
        assert_eq!(queue_man.next(), None);

    }
}
