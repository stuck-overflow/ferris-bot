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
pub enum QueueManagerJoinError {
    QueueFull,
    UserAlreadyInQueue,
}

pub enum QueueManagerLeaveError {
    UserNotInQueue,
}

impl QueueManager {
    pub fn new(capacity: usize) -> QueueManager {
        QueueManager {
            queue: VecDeque::new(),
            capacity,
        }
    }

    pub fn join(&mut self, name: &str, user_type: UserType) -> Result<(), QueueManagerJoinError> {
        if self.queue.iter().any(|x| x == name) {
            return Err(QueueManagerJoinError::UserAlreadyInQueue);
        }
        if self.queue.len() == self.capacity {
            return Err(QueueManagerJoinError::QueueFull);
        }
        self.queue.push_back(String::from(name));
        Ok(())
    }

    pub fn queue(&self) -> impl Iterator<Item = &String> {
        self.queue.iter()
    }

    pub fn next(&mut self) -> Option<String> {
        self.queue.pop_front()
    }

    pub fn leave(&mut self, name: &str) -> Result<(), QueueManagerLeaveError> {
        match self.queue.iter().position(|x| x == name) {
            Some(i) => {
                self.queue.remove(i);
                Ok(())
            }
            None => Err(QueueManagerLeaveError::UserNotInQueue),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{distributions::Alphanumeric, thread_rng, Rng};
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
            assert!(queue_man.join(&random_user, UserType::Default).is_ok());
            // Second invocation with same user should fail.
            let result = queue_man.join(&random_user, UserType::Default);

            assert!(matches!(
                result,
                Err(QueueManagerJoinError::UserAlreadyInQueue)
            ));
            users.push(random_user);
        }
        let random_user = gen_random_user();
        // Queue should have reached capacity by now, so any new user should fail.
        let result = queue_man.join(&random_user, UserType::Default);
        assert!(matches!(result, Err(QueueManagerJoinError::QueueFull)));

        // call next(), the user should be users.get(0);
        for i in 0..3 {
            assert_eq!(queue_man.next(), Some(users.get(i).unwrap().to_owned()));
        }
        assert_eq!(queue_man.next(), None);
    }

    #[test]
    fn test_queue_leave() {
        let mut queue_man = QueueManager::new(3);
        // somebody should join the queue
        let random_user_1 = gen_random_user();
        let random_user_2 = gen_random_user();
        assert!(queue_man.join(&random_user_1, UserType::Default).is_ok());
        assert!(queue_man.join(&random_user_2, UserType::Default).is_ok());
        assert!(queue_man.queue().any(|x| x == &random_user_1));
        assert!(queue_man.queue().any(|x| x == &random_user_2));
        // assert they leave the queue
        assert!(queue_man.leave(&random_user_2).is_ok());
        assert!(queue_man.queue().any(|x| x == &random_user_1));
        assert!(!queue_man.queue().any(|x| x == &random_user_2));
        assert!(matches!(
            queue_man.leave(&random_user_2),
            Err(QueueManagerLeaveError::UserNotInQueue)
        ))
        // assert that leave() returns error if user not in queue
    }
}
