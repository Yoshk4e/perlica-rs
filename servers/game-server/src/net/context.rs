use crate::player::Player;
use config::BeyondAssets;
use perlica_db::PlayerDb;
use perlica_proto::{CsHead, NetMessage, prost::Message};
use tokio::sync::mpsc;

pub struct NetContext<'a> {
    // This is our network context, holding all the essential info for handling a network request. It's like a temporary briefcase for each transaction.
    pub player: &'a mut Player, // A mutable reference to the player. We're gonna be changing their state, so gotta be able to write to it.
    pub db: &'static PlayerDb, // Reference to the database. Gotta save and load player data, ya know?
    pub client_seq_id: u64,    // The sequence ID from the client. Helps keep things in order.
    pub assets: &'static BeyondAssets, // Reference to game assets. All the cool items, characters, etc.
    outbound: &'a mpsc::Sender<Vec<u8>>, // The sender for outbound messages. How we talk back to the client.
    server_seq_id: &'a mut u64, // Our server's sequence ID. Gotta keep track of our own messages too.
}

impl<'a> NetContext<'a> {
    // Methods for our network context. Where the magic happens, again! XD
    pub fn new(
        // Constructor for a new NetContext. Setting up the briefcase.
        player: &'a mut Player,
        db: &'static PlayerDb,
        assets: &'static BeyondAssets,
        outbound: &'a mpsc::Sender<Vec<u8>>,
        client_seq_id: u64,
        server_seq_id: &'a mut u64,
    ) -> Self {
        Self {
            player,
            db,
            assets,
            outbound,
            client_seq_id,
            server_seq_id,
        }
    }

    pub async fn send<T: NetMessage>(&mut self, message: T) -> std::io::Result<()> {
        // Sending a message back to the client. Don't forget the return receipt!
        self.write_frame(message, true).await
    }

    pub async fn notify<T: NetMessage>(&mut self, message: T) -> std::io::Result<()> {
        // Sending a notification to the client. No response expected, just a heads-up.
        self.write_frame(message, false).await
    }

    async fn write_frame<T: NetMessage>(
        // This is the core function for writing framed messages to the client. It's like the post office for our game data.
        &mut self,
        message: T,
        is_response: bool,
    ) -> std::io::Result<()> {
        let body = message.encode_to_vec(); // Encoding the message body into bytes. Ready to be shipped!

        let head = CsHead {
            // Creating the message header. Gotta make it official.
            msgid: T::CMD_ID,
            up_seqid: self.client_seq_id,
            down_seqid: *self.server_seq_id,
        };
        let head_bytes = head.encode_to_vec(); // Encoding the header into bytes.

        let mut frame = Vec::with_capacity(3 + head_bytes.len() + body.len()); // Allocating space for the entire frame. Efficiency is key!
        frame.push(head_bytes.len() as u8); // Writing the header size. First byte, important stuff.
        frame.extend_from_slice(&(body.len() as u16).to_le_bytes()); // Writing the body size. Little-endian, because that's how we roll.
        frame.extend_from_slice(&head_bytes); // Adding the actual header bytes.
        frame.extend_from_slice(&body); // Adding the actual body bytes. The payload!

        if is_response {
            // If this is a response, increment the server sequence ID.
            *self.server_seq_id += 1; // Incrementing the server sequence ID. Keep track of those responses!
        }

        self.outbound
            .send(frame) // Sending the complete frame! Adios, message!
            .await
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::BrokenPipe, "write task closed")) // Oh no, the write task closed! That's a broken pipe, fam. XD
    }
}
