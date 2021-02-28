use std::collections::VecDeque;

pub struct QueueManager {
    queue_users: VecDeque<String>,
    queue_subscribers: VecDeque<String>,
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
            queue_users: VecDeque::new(),
            queue_subscribers: VecDeque::new(),
            capacity,
        }
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
        Ok(())
    }

    pub fn queue(&self) -> impl Iterator<Item = &String> {
        // Subscribers are always at the beginning of the queue.
        self.queue_subscribers.iter().chain(self.queue_users.iter())
    }

    pub fn next(&mut self) -> Option<String> {
        if self.queue_subscribers.len() > 0 {
            self.queue_subscribers.pop_front()
        } else {
            self.queue_users.pop_front()
        }
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
        let mut queue_man = QueueManager::new(6);
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
        for i in 0..3 {
            assert_eq!(
                queue_man.next(),
                Some(subscribers.get(i).unwrap().to_owned())
            );
        }
        // next we should see the other users.
        for i in 0..3 {
            assert_eq!(queue_man.next(), Some(users.get(i).unwrap().to_owned()));
        }
        assert_eq!(queue_man.next(), None);
    }

    #[test]
    fn test_queue_leave() {
        let mut queue_man = QueueManager::new(3);

        let random_user_1 = gen_random_user();
        let random_user_2 = gen_random_user();
        let random_user_3 = gen_random_user();
        assert!(queue_man.join(&random_user_1, UserType::Default).is_ok());
        assert!(queue_man.join(&random_user_2, UserType::Default).is_ok());
        assert!(queue_man.join(&random_user_3, UserType::Subscriber).is_ok());
        assert!(queue_man.queue().any(|x| x == &random_user_1));
        assert!(queue_man.queue().any(|x| x == &random_user_2));
        assert!(queue_man.queue().any(|x| x == &random_user_3));

        assert!(queue_man.leave(&random_user_2).is_ok());
        assert!(queue_man.queue().any(|x| x == &random_user_1));
        assert!(!queue_man.queue().any(|x| x == &random_user_2));
        assert!(queue_man.queue().any(|x| x == &random_user_3));

        assert!(matches!(
            queue_man.leave(&random_user_2),
            Err(QueueManagerLeaveError::UserNotInQueue)
        ));

        assert!(queue_man.leave(&random_user_3).is_ok());
        assert!(queue_man.queue().any(|x| x == &random_user_1));
        assert!(!queue_man.queue().any(|x| x == &random_user_2));
        assert!(!queue_man.queue().any(|x| x == &random_user_3));

        assert!(matches!(
            queue_man.leave(&random_user_3),
            Err(QueueManagerLeaveError::UserNotInQueue)
        ));
    }
}
