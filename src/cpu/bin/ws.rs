use actix::{fut, ActorContext};
use crate::messages::{Disconnect, Connect, WsMessage, ClientActorMessage};
use crate::lobby::Lobby;
use actix::{Actor, Addr, Running, StreamHandler, WrapFuture, ActorFuture, ContextFutureSpawner};
use actix::{AsyncContext, Handler};
use actix_web_actors::ws;
use actix_web_actors::ws::Message::Text;
use std::time::{Duration, Instant};
use actix_web::guard::Connect;
use actix_web_actors::ws::{Message, ProtocolError};
use uuid::Uuid;

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(100);
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);

pub struct WsConn {
    room: Uuid,
    lobby_addr: Addr<Lobby>,
    hb: Instant,
    id: Uuid
}

impl WsConn {
    pub fn new(room: Uuid, lobby: Addr<Lobby>) -> Self {
        WsConn {
            id: Uuid::new_v4(),
            room,
            hb: Instant::now(),
            lobby_addr: lobby
        }
    }

    fn hb(&self, ctx: &mut Actor::Context) {
        ctx.run_interval(HEARTBEAT_INTERVAL, |act, ctx| {
            ctx.text("센서 헬스 체크");
            if Instant::now().duration_since(act.hb) > CLIENT_TIMEOUT {
                println!("disconnect due to failed heartbeat");
                act.lobby_addr.do_send(Disconnect {
                    id: self.id,
                    room_id: self.room
                });
                ctx.stop();
                return;
            }

            ctx.pong(b"PING");
        });
    }
}

impl Actor for WsConn {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        self.hb(ctx);

        let addr = ctx.address();
        self.lobby_addr
            .send(Connect {
                addr: addr.recipient(),
                lobby_id: self.room,
                self_id: self.id
            })
            .into_actor(self)
            .then(|res, _, ctx| {
                match res {
                    Ok(_res) => (),
                    _ => ctx.stop()
                }
                fut::ready(())
            })
            .wait(ctx);
    }

    fn stopped(&mut self, ctx: &mut Self::Context) -> Running {
        self.lobby_addr.do_send(Disconnect {
            id: self.id,
            room_id: self.room
        });
        Running::Stop
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WsConn {
    fn handle(&mut self, item: Result<Message, ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Ping(msg)) => {
                self.hb = Instant::now();
                ctx.pong(&msg);
            }
            Ok(ws::Message::Pong(_)) => self.hb = Instant::now(),
            Ok(ws::Message::Binary(bin)) => ctx.binary(bin),
            Ok(ws::Message::Close(reason)) => {
                ctx.close(reason);
                ctx.stop();
            },
            Ok(ws::Message::Continuation(_)) => {
                ctx.stop();
            }
            Ok(ws::Message::Nop) => (),
            Ok(ws::Message::Text(text)) => {
                self.lobby_addr.do_send(ClientActorMessage {
                    id: self.id,
                    msg: text,
                    room_id: self.room
                })
            },
            Err(e) => panic!(e)
        }
    }
}

impl Handler<WsMessage> for WsConn {
    type Result = ();

    fn handle(&mut self, msg: WsMessage, ctx: &mut Self::Context) {
        ctx.text(msg);
    }
}