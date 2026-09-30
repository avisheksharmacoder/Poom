use iced::Subscription;
use poom_daemon::EventBroadcaster;

use crate::state::Message;

pub fn live_spans_subscription(
    broadcaster: Option<EventBroadcaster>,
) -> Subscription<Message> {
    let Some(broadcaster) = broadcaster else {
        return Subscription::none();
    };

    Subscription::run_with_id("live_spans_stream", iced::stream::channel(100, move |mut output| async move {
        use futures_util::SinkExt;
        let mut rx = broadcaster.subscribe();

        while let Ok(span) = rx.recv().await {
            let _ = output.send(Message::LiveSpanReceived(span)).await;
        }
    }))
}
