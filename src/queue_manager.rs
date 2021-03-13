use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fs;

#[derive(Debug, Deserialize, Serialize)]
pub struct QueueManager {
    queue_users: VecDeque<String>,
    queue_subscribers: VecDeque<String>,
    capacity: usize,
    storage_file_path: String,
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
    pub fn new(capacity: usize, storage_file_path: &str) -> QueueManager {
        if storage_file_path.is_empty() {
            panic!("Must specify a file to store the queue data.");
        }
        let queue_manager = fs::read_to_string(storage_file_path);
        if queue_manager.is_err() {
            return QueueManager {
                queue_users: VecDeque::new(),
                queue_subscribers: VecDeque::new(),
                capacity,
                storage_file_path: String::from(storage_file_path),
            };
        }
        serde_json::from_str::<QueueManager>(&queue_manager.unwrap()).unwrap()
    }

    fn update_storage(&self) {
        let content = serde_json::to_string(self).unwrap();
        fs::write(self.storage_file_path.to_owned(), content).expect("Unable to write file");
    }

    pub fn join(&mut self, name: &str, user_type: UserType) -> Result<(), QueueManagerJoinError> {
        if self.queue_subscribers.iter().any(|x| x == name)
            || self.queue_users.iter().any(|x| x == name)
        {
            return Err(QueueManagerJoinError::UserAlreadyInQueue);
        }
        if (self.queue_subscribers.len() + self.queue_users.len()) == self.capacity {
            return Err(QueueManagerJoinError::QueueFull);
        }
        match user_type {
            UserType::Default => self.queue_users.push_back(String::from(name)),
            UserType::Subscriber => self.queue_subscribers.push_back(String::from(name)),
        }
        self.update_storage();
        Ok(())
    }

    pub fn queue(&self) -> impl Iterator<Item = &String> {
        // Subscribers are always at the beginning of the queue.
        self.queue_subscribers.iter().chain(self.queue_users.iter())
    }

    pub fn next(&mut self) -> Option<String> {
        let res = if self.queue_subscribers.len() > 0 {
            self.queue_subscribers.pop_front()
        } else {
            self.queue_users.pop_front()
        };
        self.update_storage();
        res
    }

    fn remove_from_queue(
        queue: &mut VecDeque<String>,
        name: &str,
    ) -> Result<(), QueueManagerLeaveError> {
        match queue.iter().position(|x| x == name) {
            Some(i) => {
                queue.remove(i);
                Ok(())
            }
            None => Err(QueueManagerLeaveError::UserNotInQueue),
        }
    }

    pub fn leave(&mut self, name: &str) -> Result<(), QueueManagerLeaveError> {
        QueueManager::remove_from_queue(&mut self.queue_subscribers, name)
            .or_else(|_| QueueManager::remove_from_queue(&mut self.queue_users, name))
    }

    pub fn kick(&mut self, name: &str) -> Result<(), QueueManagerLeaveError> {
        self.leave(name)
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
        let mut subscribers = vec![];
        fs::remove_file("storage1.json").unwrap();
        let mut queue_man = QueueManager::new(6, "storage1.json");
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

            let random_subscriber = gen_random_user();
            assert!(queue_man
                .join(&random_subscriber, UserType::Subscriber)
                .is_ok());
            // Second invocation with same user should fail.
            let result = queue_man.join(&random_subscriber, UserType::Subscriber);
            assert!(matches!(
                result,
                Err(QueueManagerJoinError::UserAlreadyInQueue)
            ));
            subscribers.push(random_subscriber);
        }
        let random_user = gen_random_user();
        // Queue should have reached capacity by now, so any new user should fail.
        let result = queue_man.join(&random_user, UserType::Default);
        assert!(matches!(result, Err(QueueManagerJoinError::QueueFull)));

        // first in queue should be the subscribers.
        dbg!(&queue_man);
        for i in 0..3 {
            assert_eq!(
                queue_man.next(),
                Some(subscribers.get(i).unwrap().to_owned())
            );
            dbg!(&queue_man);
        }
        // next we should see the other users.
        for i in 0..3 {
            let mut queue_man = QueueManager::new(6, "storage1.json");
            assert_eq!(queue_man.next(), Some(users.get(i).unwrap().to_owned()));
            dbg!(&queue_man);
        }
        let mut queue_man = QueueManager::new(6, "storage1.json");
        assert_eq!(queue_man.next(), None);
        dbg!(&queue_man);
    }

    #[test]
    fn test_queue_leave() {
        fs::remove_file("storage2.json").unwrap();
        let capacity = 4;
        let mut queue_man = QueueManager::new(capacity, "storage2.json");

        let random_user_1 = gen_random_user();
        let random_user_2 = gen_random_user();
        let random_user_3 = gen_random_user();
        let random_user_4 = gen_random_user();
        assert!(queue_man.join(&random_user_1, UserType::Default).is_ok());
        assert!(queue_man.join(&random_user_2, UserType::Default).is_ok());
        assert!(queue_man.join(&random_user_3, UserType::Subscriber).is_ok());
        assert!(queue_man.join(&random_user_4, UserType::Default).is_ok());
        assert!(queue_man.queue().any(|x| x == &random_user_1));
        assert!(queue_man.queue().any(|x| x == &random_user_2));
        assert!(queue_man.queue().any(|x| x == &random_user_3));
        assert!(queue_man.queue().any(|x| x == &random_user_4));

        let mut queue_man = QueueManager::new(capacity, "storage2.json");

        assert!(queue_man.leave(&random_user_2).is_ok());
        assert!(queue_man.queue().any(|x| x == &random_user_1));
        assert!(!queue_man.queue().any(|x| x == &random_user_2));
        assert!(queue_man.queue().any(|x| x == &random_user_3));
        assert!(queue_man.queue().any(|x| x == &random_user_4));

        assert!(matches!(
            queue_man.leave(&random_user_2),
            Err(QueueManagerLeaveError::UserNotInQueue)
        ));

        assert!(queue_man.leave(&random_user_3).is_ok());
        assert!(queue_man.queue().any(|x| x == &random_user_1));
        assert!(!queue_man.queue().any(|x| x == &random_user_2));
        assert!(!queue_man.queue().any(|x| x == &random_user_3));
        assert!(queue_man.queue().any(|x| x == &random_user_4));

        assert!(matches!(
            queue_man.leave(&random_user_3),
            Err(QueueManagerLeaveError::UserNotInQueue)
        ));

        assert!(queue_man.kick(&random_user_4).is_ok());
        assert!(queue_man.queue().any(|x| x == &random_user_1));
        assert!(!queue_man.queue().any(|x| x == &random_user_2));
        assert!(!queue_man.queue().any(|x| x == &random_user_3));
        assert!(!queue_man.queue().any(|x| x == &random_user_4));

        assert!(matches!(
            queue_man.leave(&random_user_4),
            Err(QueueManagerLeaveError::UserNotInQueue)
        ));
    }
}
