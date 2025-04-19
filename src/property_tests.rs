use proptest::prelude::*;
use crate::types::*;
use std::convert::TryInto;

// Define generators for A2A types
prop_compose! {
    fn arb_task_state()(i in 0..7) -> TaskState {
        match i {
            0 => TaskState::Submitted,
            1 => TaskState::Working,
            2 => TaskState::InputRequired,
            3 => TaskState::Completed,
            4 => TaskState::Canceled,
            5 => TaskState::Failed,
            _ => TaskState::Unknown,
        }
    }
}

prop_compose! {
    fn arb_role()(i in 0..2) -> Role {
        match i {
            0 => Role::User,
            _ => Role::Agent,
        }
    }
}

prop_compose! {
    fn arb_text_part()(text in ".*") -> TextPart {
        TextPart {
            type_: "text".to_string(),
            text,
            metadata: None,
        }
    }
}

prop_compose! {
    fn arb_part()(text_part in arb_text_part()) -> Part {
        Part::TextPart(text_part)
    }
}

prop_compose! {
    fn arb_message()(role in arb_role(), 
                     parts in prop::collection::vec(arb_part(), 1..3)) -> Message {
        Message {
            role,
            parts,
            metadata: None,
        }
    }
}

// Example property test
pub fn run_property_tests(count: usize) {
    // Configure proptest with desired number of cases
    let config = ProptestConfig::with_cases(count.try_into().unwrap_or(100));

    // Use closures instead of test blocks in proptest
    proptest!(config, |(state in arb_task_state(), message in arb_message())| {
        // Create a task status
        let status = TaskStatus {
            state: state.clone(),
            message: Some(message.clone()),
            timestamp: Some(chrono::Utc::now()),
        };

        // Test serialization/deserialization roundtrip
        let json = serde_json::to_string(&status).unwrap();
        let deserialized: TaskStatus = serde_json::from_str(&json).unwrap();
        
        // Property: should roundtrip correctly
        prop_assert_eq!(status.state, deserialized.state);
        
        // If message exists, check that too
        if let Some(orig_msg) = &status.message {
            if let Some(deser_msg) = &deserialized.message {
                prop_assert_eq!(orig_msg.role, deser_msg.role);
            }
        }

        println!("Test passed for state: {:?}", state);
    });

    println!("All {} property tests passed!", count);
}
