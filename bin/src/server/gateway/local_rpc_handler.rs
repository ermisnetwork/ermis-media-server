use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
};

use atm0s_sdn::NodeId;
use media_server_connector::agent_service::Control as ConnectorControl;
use media_server_gateway::ServiceKind;
use media_server_protocol::{
    endpoint::ClusterConnId,
    gateway::GATEWAY_RPC_PORT,
    multi_tenancy::AppContext,
    protobuf::{
        cluster_connector::peer_event::RouteBegin,
        cluster_gateway::MediaEdgeServiceClient,
        gateway::{ConnectRequest, ConnectResponse, RemoteIceRequest, RemoteIceResponse},
    },
    rpc::{
        node_vnet_addr,
        quinn::{QuinnClient, QuinnStream},
    },
    transport::{
        rtpengine::{RtpCreateAnswerRequest, RtpCreateOfferRequest},
        webrtc,
        whep::{self, WhepConnectReq, WhepConnectRes, WhepDeleteReq, WhepDeleteRes, WhepRemoteIceReq, WhepRemoteIceRes},
        whip::{self, WhipConnectReq, WhipConnectRes, WhipDeleteReq, WhipDeleteRes, WhipRemoteIceReq, WhipRemoteIceRes},
        RpcError, RpcReq, RpcRes, RpcResult,
    },
};
use media_server_protocol::{
    protobuf::{
        cluster_connector::{
            connector_request::Request as ConnectorRequest,
            peer_event::{route_error::ErrorType, Event as PeerEvent2, RouteError, RouteSuccess},
            PeerEvent,
        },
        cluster_gateway::WhipConnectRequest,
    },
    transport::rtpengine,
};
use media_server_utils::now_ms;
use tokio::sync::mpsc::Sender;

use crate::errors::MediaServerError;

use super::{dest_selector::GatewayDestSelector, ip_location::Ip2Location};

pub struct MediaLocalRpcHandler {
    connector_agent_tx: Sender<ConnectorControl>,
    selector: GatewayDestSelector,
    client: MediaEdgeServiceClient<SocketAddr, QuinnClient, QuinnStream>,
    ip2location: Arc<Ip2Location>,
}

impl MediaLocalRpcHandler {
    async fn feedback_route_begin(&self, app: &str, session_id: u64, ip: IpAddr) {
        self.connector_agent_tx
            .send(ConnectorControl::Request(
                now_ms(),
                ConnectorRequest::Peer(PeerEvent {
                    app: app.to_owned(),
                    session_id,
                    event: Some(PeerEvent2::RouteBegin(RouteBegin { remote_ip: ip.to_string() })),
                }),
            ))
            .await
            .expect("Should send");
    }

    async fn feedback_route_success(&self, app: &str, session_id: u64, after_ms: u64, node: NodeId) {
        self.connector_agent_tx
            .send(ConnectorControl::Request(
                now_ms(),
                ConnectorRequest::Peer(PeerEvent {
                    app: app.to_owned(),
                    session_id,
                    event: Some(PeerEvent2::RouteSuccess(RouteSuccess {
                        after_ms: after_ms as u32,
                        dest_node: node,
                    })),
                }),
            ))
            .await
            .expect("Should send");
    }

    async fn feedback_route_error(&self, app: &str, session_id: u64, after_ms: u64, node: Option<NodeId>, error: ErrorType) {
        self.connector_agent_tx
            .send(ConnectorControl::Request(
                now_ms(),
                ConnectorRequest::Peer(PeerEvent {
                    app: app.to_owned(),
                    session_id,
                    event: Some(PeerEvent2::RouteError(RouteError {
                        after_ms: after_ms as u32,
                        dest_node: node,
                        error: error as i32,
                    })),
                }),
            ))
            .await
            .expect("Should send");
    }
}

impl MediaLocalRpcHandler {
    pub fn new(
        connector_agent_tx: Sender<ConnectorControl>,
        selector: GatewayDestSelector,
        client: MediaEdgeServiceClient<SocketAddr, QuinnClient, QuinnStream>,
        ip2location: Arc<Ip2Location>,
    ) -> Self {
        Self {
            connector_agent_tx,
            selector,
            client,
            ip2location,
        }
    }

    pub async fn process_req(&self, conn_part: Option<(NodeId, u64)>, param: RpcReq<ClusterConnId>) -> RpcRes<ClusterConnId> {
        match param {
            RpcReq::Whip(param) => match param {
                whip::RpcReq::Connect(param) => RpcRes::Whip(whip::RpcRes::Connect(self.whip_connect(param).await)),
                whip::RpcReq::RemoteIce(param) => RpcRes::Whip(whip::RpcRes::RemoteIce(self.whip_remote_ice(conn_part, param).await)),
                whip::RpcReq::Delete(param) => RpcRes::Whip(whip::RpcRes::Delete(self.whip_delete(conn_part, param).await)),
            },
            RpcReq::Whep(param) => match param {
                whep::RpcReq::Connect(param) => RpcRes::Whep(whep::RpcRes::Connect(self.whep_connect(param).await)),
                whep::RpcReq::RemoteIce(param) => RpcRes::Whep(whep::RpcRes::RemoteIce(self.whep_remote_ice(conn_part, param).await)),
                whep::RpcReq::Delete(param) => RpcRes::Whep(whep::RpcRes::Delete(self.whep_delete(conn_part, param).await)),
            },
            RpcReq::Webrtc(param) => match param {
                webrtc::RpcReq::Connect(app, session_id, ip, user_agent, param, extra_data, record) => {
                    RpcRes::Webrtc(webrtc::RpcRes::Connect(self.webrtc_connect(session_id, app, ip, user_agent, param, extra_data, record).await))
                }
                webrtc::RpcReq::RemoteIce(conn, param) => RpcRes::Webrtc(webrtc::RpcRes::RemoteIce(self.webrtc_remote_ice(conn_part, conn, param).await)),
                webrtc::RpcReq::RestartIce(conn, app, ip, user_agent, req, extra_data, record) => {
                    RpcRes::Webrtc(webrtc::RpcRes::RestartIce(self.webrtc_restart_ice(conn_part, conn, app, ip, user_agent, req, extra_data, record).await))
                }
                webrtc::RpcReq::Delete(_) => {
                    //TODO implement delete webrtc conn
                    RpcRes::Webrtc(webrtc::RpcRes::RestartIce(Err(RpcError::new2(MediaServerError::NotImplemented))))
                }
            },
            RpcReq::RtpEngine(param) => match param {
                rtpengine::RpcReq::CreateOffer(param) => RpcRes::RtpEngine(rtpengine::RpcRes::CreateOffer(self.rtpengine_create_offer(param).await)),
                rtpengine::RpcReq::SetAnswer(conn, param) => RpcRes::RtpEngine(rtpengine::RpcRes::SetAnswer(self.rtpengine_set_answer(conn_part, conn, param.sdp).await)),
                rtpengine::RpcReq::CreateAnswer(param) => RpcRes::RtpEngine(rtpengine::RpcRes::CreateAnswer(self.rtpengine_create_answer(param).await)),
                rtpengine::RpcReq::Delete(param) => RpcRes::RtpEngine(rtpengine::RpcRes::Delete(self.rtpengine_delete(conn_part, param).await)),
            },
        }
    }

    /*
        Whip part
    */

    async fn whip_connect(&self, param: WhipConnectReq) -> RpcResult<WhipConnectRes<ClusterConnId>> {
        let session_id = param.session_id;
        let started_at = now_ms();
        self.feedback_route_begin(&param.app.app, session_id, param.ip).await;

        if let Some(node_id) = self.selector.select(ServiceKind::Webrtc, self.ip2location.get_location(&param.ip)).await {
            let sock_addr = node_vnet_addr(node_id, GATEWAY_RPC_PORT);
            log::info!("[Gateway] selected node {node_id}");
            let mut rpc_req: WhipConnectRequest = param.clone().into();
            rpc_req.session_id = session_id;

            let res = self.client.whip_connect(sock_addr, rpc_req).await;
            log::info!("[Gateway] response from node {node_id} => {:?}", res);
            if let Some(res) = res {
                self.feedback_route_success(&param.app.app, session_id, now_ms() - started_at, node_id).await;

                Ok(whip::WhipConnectRes {
                    sdp: res.sdp,
                    conn_id: res.conn.parse().unwrap(),
                })
            } else {
                self.feedback_route_error(&param.app.app, session_id, now_ms() - started_at, Some(node_id), ErrorType::Timeout).await;
                Err(RpcError::new2(MediaServerError::GatewayRpcError))
            }
        } else {
            self.feedback_route_error(&param.app.app, session_id, now_ms() - started_at, None, ErrorType::PoolEmpty).await;
            Err(RpcError::new2(MediaServerError::NodePoolEmpty))
        }
    }

    async fn whip_remote_ice(&self, conn_part: Option<(NodeId, u64)>, param: WhipRemoteIceReq<ClusterConnId>) -> RpcResult<WhipRemoteIceRes> {
        if let Some((node, _session)) = conn_part {
            let rpc_req = media_server_protocol::protobuf::cluster_gateway::WhipRemoteIceRequest {
                conn: param.conn_id.to_string(),
                ice: param.ice,
            };
            log::info!("[Gateway] selected node {node}");
            let sock_addr = node_vnet_addr(node, GATEWAY_RPC_PORT);
            let res = self.client.whip_remote_ice(sock_addr, rpc_req).await;
            if let Some(_res) = res {
                Ok(whip::WhipRemoteIceRes {})
            } else {
                Err(RpcError::new2(MediaServerError::GatewayRpcError))
            }
        } else {
            Err(RpcError::new2(MediaServerError::InvalidConnId))
        }
    }

    async fn whip_delete(&self, conn_part: Option<(NodeId, u64)>, param: WhipDeleteReq<ClusterConnId>) -> RpcResult<WhipDeleteRes> {
        if let Some((node, _session)) = conn_part {
            let rpc_req = media_server_protocol::protobuf::cluster_gateway::WhipCloseRequest { conn: param.conn_id.to_string() };
            log::info!("[Gateway] selected node {node}");
            let sock_addr = node_vnet_addr(node, GATEWAY_RPC_PORT);
            let res = self.client.whip_close(sock_addr, rpc_req).await;
            if let Some(_res) = res {
                Ok(whip::WhipDeleteRes {})
            } else {
                Err(RpcError::new2(MediaServerError::GatewayRpcError))
            }
        } else {
            Err(RpcError::new2(MediaServerError::InvalidConnId))
        }
    }

    /*
        Whep part
    */

    async fn whep_connect(&self, param: WhepConnectReq) -> RpcResult<WhepConnectRes<ClusterConnId>> {
        let started_at = now_ms();
        let session_id = param.session_id;
        self.feedback_route_begin(&param.app.app, session_id, param.ip).await;

        if let Some(node_id) = self.selector.select(ServiceKind::Webrtc, self.ip2location.get_location(&param.ip)).await {
            let sock_addr = node_vnet_addr(node_id, GATEWAY_RPC_PORT);
            log::info!("[Gateway] selected node {node_id}");
            let res = self.client.whep_connect(sock_addr, param.clone().into()).await;
            log::info!("[Gateway] response from node {node_id} => {:?}", res);
            if let Some(res) = res {
                self.feedback_route_success(&param.app.app, session_id, now_ms() - started_at, node_id).await;
                Ok(whep::WhepConnectRes {
                    sdp: res.sdp,
                    conn_id: res.conn.parse().unwrap(),
                })
            } else {
                self.feedback_route_error(&param.app.app, session_id, now_ms() - started_at, Some(node_id), ErrorType::Timeout).await;
                Err(RpcError::new2(MediaServerError::GatewayRpcError))
            }
        } else {
            self.feedback_route_error(&param.app.app, session_id, now_ms() - started_at, None, ErrorType::PoolEmpty).await;
            Err(RpcError::new2(MediaServerError::NodePoolEmpty))
        }
    }

    async fn whep_remote_ice(&self, conn_part: Option<(NodeId, u64)>, param: WhepRemoteIceReq<ClusterConnId>) -> RpcResult<WhepRemoteIceRes> {
        if let Some((node, _session)) = conn_part {
            let rpc_req = media_server_protocol::protobuf::cluster_gateway::WhepRemoteIceRequest {
                conn: param.conn_id.to_string(),
                ice: param.ice,
            };
            log::info!("[Gateway] selected node {node}");
            let sock_addr = node_vnet_addr(node, GATEWAY_RPC_PORT);
            let res = self.client.whep_remote_ice(sock_addr, rpc_req).await;
            if let Some(_res) = res {
                Ok(whep::WhepRemoteIceRes {})
            } else {
                Err(RpcError::new2(MediaServerError::GatewayRpcError))
            }
        } else {
            Err(RpcError::new2(MediaServerError::InvalidConnId))
        }
    }

    async fn whep_delete(&self, conn_part: Option<(NodeId, u64)>, param: WhepDeleteReq<ClusterConnId>) -> RpcResult<WhepDeleteRes> {
        if let Some((node, _session)) = conn_part {
            let rpc_req = media_server_protocol::protobuf::cluster_gateway::WhepCloseRequest { conn: param.conn_id.to_string() };
            log::info!("[Gateway] selected node {node}");
            let sock_addr = node_vnet_addr(node, GATEWAY_RPC_PORT);
            let res = self.client.whep_close(sock_addr, rpc_req).await;
            if let Some(_res) = res {
                Ok(whep::WhepDeleteRes {})
            } else {
                Err(RpcError::new2(MediaServerError::GatewayRpcError))
            }
        } else {
            Err(RpcError::new2(MediaServerError::InvalidConnId))
        }
    }

    /*
    Webrtc part
    */

    #[allow(clippy::too_many_arguments)]
    async fn webrtc_connect(
        &self,
        session_id: u64,
        app: AppContext,
        ip: IpAddr,
        user_agent: String,
        req: ConnectRequest,
        extra_data: Option<String>,
        record: bool,
    ) -> RpcResult<(ClusterConnId, ConnectResponse)> {
        let started_at = now_ms();
        self.feedback_route_begin(&app.app, session_id, ip).await;

        if let Some(node_id) = self.selector.select(ServiceKind::Webrtc, self.ip2location.get_location(&ip)).await {
            let sock_addr = node_vnet_addr(node_id, GATEWAY_RPC_PORT);
            log::info!("[Gateway] selected node {node_id}");
            let rpc_req = media_server_protocol::protobuf::cluster_gateway::WebrtcConnectRequest {
                app: Some(app.clone().into()),
                session_id,
                user_agent,
                ip: ip.to_string(),
                req: Some(req),
                record,
                extra_data,
            };
            let res = self.client.webrtc_connect(sock_addr, rpc_req).await;
            log::info!("[Gateway] response from node {node_id} => {:?}", res);
            if let Some(res) = res {
                if let Some(res) = res.res {
                    if let Ok(conn) = res.conn_id.parse() {
                        self.feedback_route_success(&app.app, session_id, now_ms() - started_at, node_id).await;
                        Ok((conn, res))
                    } else {
                        self.feedback_route_error(&app.app, session_id, now_ms() - started_at, Some(node_id), ErrorType::MediaError).await;
                        Err(RpcError::new2(MediaServerError::MediaResError))
                    }
                } else {
                    self.feedback_route_error(&app.app, session_id, now_ms() - started_at, Some(node_id), ErrorType::GatewayError).await;
                    Err(RpcError::new2(MediaServerError::GatewayRpcError))
                }
            } else {
                self.feedback_route_error(&app.app, session_id, now_ms() - started_at, Some(node_id), ErrorType::Timeout).await;
                Err(RpcError::new2(MediaServerError::NodeTimeout))
            }
        } else {
            self.feedback_route_error(&app.app, session_id, now_ms() - started_at, None, ErrorType::PoolEmpty).await;
            Err(RpcError::new2(MediaServerError::NodePoolEmpty))
        }
    }

    async fn webrtc_remote_ice(&self, conn_part: Option<(NodeId, u64)>, conn: ClusterConnId, param: RemoteIceRequest) -> RpcResult<RemoteIceResponse> {
        let (node, _session) = conn_part.ok_or(RpcError::new2(MediaServerError::InvalidConnId))?;
        let rpc_req = media_server_protocol::protobuf::cluster_gateway::WebrtcRemoteIceRequest {
            conn: conn.to_string(),
            candidates: param.candidates,
        };
        log::info!("[Gateway] selected node {node}");
        let sock_addr = node_vnet_addr(node, GATEWAY_RPC_PORT);
        let res = self.client.webrtc_remote_ice(sock_addr, rpc_req).await;
        let res = res.ok_or(RpcError::new2(MediaServerError::GatewayRpcError))?;
        Ok(RemoteIceResponse { added: res.added })
    }

    #[allow(clippy::too_many_arguments)]
    async fn webrtc_restart_ice(
        &self,
        conn_part: Option<(NodeId, u64)>,
        conn: ClusterConnId,
        app: AppContext,
        ip: IpAddr,
        user_agent: String,
        req: ConnectRequest,
        extra_data: Option<String>,
        record: bool,
    ) -> RpcResult<(ClusterConnId, ConnectResponse)> {
        let (node, _session) = conn_part.ok_or(RpcError::new2(MediaServerError::InvalidConnId))?;
        let dest = match self.selector.dest_for(ServiceKind::Webrtc, node).await {
            Some(dest) => dest,
            None => match self.selector.select(ServiceKind::Webrtc, self.ip2location.get_location(&ip)).await {
                Some(dest) => {
                    log::warn!("[Gateway] not found dest {node} found other node {dest} for restart-ice (reconnect to other server)");
                    dest
                }
                None => {
                    log::warn!("[Gateway] node pool empty for restart-ice to dest {node}");
                    return RpcResult::Err(RpcError::new2(MediaServerError::NodePoolEmpty));
                }
            },
        };
        log::info!("[Gateway] selected dest node {dest} with provided node {node}");
        let rpc_req = media_server_protocol::protobuf::cluster_gateway::WebrtcRestartIceRequest {
            app: Some(app.into()),
            conn: conn.to_string(),
            ip: ip.to_string(),
            user_agent,
            req: Some(req),
            record,
            extra_data,
        };
        let sock_addr = node_vnet_addr(dest, GATEWAY_RPC_PORT);
        let res = self.client.webrtc_restart_ice(sock_addr, rpc_req).await;
        let res = res.ok_or(RpcError::new2(MediaServerError::GatewayRpcError))?;
        let res = res.res.ok_or(RpcError::new2(MediaServerError::MediaResError))?;
        Ok((res.conn_id.parse().unwrap(), res))
    }

    /*
        RtpEngine part
    */

    async fn rtpengine_create_offer(&self, param: RtpCreateOfferRequest) -> RpcResult<(ClusterConnId, String)> {
        let started_at = now_ms();
        let session_id = param.session_id;
        // TODO get remote ip
        self.feedback_route_begin(&param.app.app, session_id, IpAddr::V4(Ipv4Addr::LOCALHOST)).await;

        if let Some(node_id) = self.selector.select(ServiceKind::RtpEngine, None).await {
            let sock_addr = node_vnet_addr(node_id, GATEWAY_RPC_PORT);
            log::info!("[Gateway] selected node {node_id}");
            let res = self.client.rtp_engine_create_offer(sock_addr, param.clone().into()).await;
            log::info!("[Gateway] response from node {node_id} => {:?}", res);
            if let Some(res) = res {
                self.feedback_route_success(&param.app.app, session_id, now_ms() - started_at, node_id).await;
                Ok((res.conn.parse().unwrap(), res.sdp))
            } else {
                self.feedback_route_error(&param.app.app, session_id, now_ms() - started_at, Some(node_id), ErrorType::Timeout).await;
                Err(RpcError::new2(MediaServerError::GatewayRpcError))
            }
        } else {
            self.feedback_route_error(&param.app.app, session_id, now_ms() - started_at, None, ErrorType::PoolEmpty).await;
            Err(RpcError::new2(MediaServerError::NodePoolEmpty))
        }
    }

    async fn rtpengine_set_answer(&self, conn_part: Option<(NodeId, u64)>, conn: ClusterConnId, sdp: String) -> RpcResult<ClusterConnId> {
        if let Some((node, _session)) = conn_part {
            let rpc_req = media_server_protocol::protobuf::cluster_gateway::RtpEngineSetAnswerRequest { conn: conn.to_string(), sdp };
            log::info!("[Gateway] selected node {node}");
            let sock_addr = node_vnet_addr(node, GATEWAY_RPC_PORT);
            let res = self.client.rtp_engine_set_answer(sock_addr, rpc_req).await;
            if let Some(_res) = res {
                Ok(conn)
            } else {
                Err(RpcError::new2(MediaServerError::GatewayRpcError))
            }
        } else {
            Err(RpcError::new2(MediaServerError::InvalidConnId))
        }
    }

    async fn rtpengine_create_answer(&self, param: RtpCreateAnswerRequest) -> RpcResult<(ClusterConnId, String)> {
        let started_at = now_ms();
        let session_id = param.session_id;
        // TODO get remote ip
        self.feedback_route_begin(&param.app.app, session_id, IpAddr::V4(Ipv4Addr::LOCALHOST)).await;

        if let Some(node_id) = self.selector.select(ServiceKind::RtpEngine, None).await {
            let sock_addr = node_vnet_addr(node_id, GATEWAY_RPC_PORT);
            log::info!("[Gateway] selected node {node_id}");
            let res = self.client.rtp_engine_create_answer(sock_addr, param.clone().into()).await;
            log::info!("[Gateway] response from node {node_id} => {:?}", res);
            if let Some(res) = res {
                self.feedback_route_success(&param.app.app, session_id, now_ms() - started_at, node_id).await;
                Ok((res.conn.parse().unwrap(), res.sdp))
            } else {
                self.feedback_route_error(&param.app.app, session_id, now_ms() - started_at, Some(node_id), ErrorType::Timeout).await;
                Err(RpcError::new2(MediaServerError::GatewayRpcError))
            }
        } else {
            self.feedback_route_error(&param.app.app, session_id, now_ms() - started_at, None, ErrorType::PoolEmpty).await;
            Err(RpcError::new2(MediaServerError::NodePoolEmpty))
        }
    }

    async fn rtpengine_delete(&self, conn_part: Option<(NodeId, u64)>, param: ClusterConnId) -> RpcResult<ClusterConnId> {
        if let Some((node, _session)) = conn_part {
            let rpc_req = media_server_protocol::protobuf::cluster_gateway::RtpEngineDeleteRequest { conn: param.to_string() };
            log::info!("[Gateway] selected node {node}");
            let sock_addr = node_vnet_addr(node, GATEWAY_RPC_PORT);
            let res = self.client.rtp_engine_delete(sock_addr, rpc_req).await;
            if let Some(_res) = res {
                Ok(param)
            } else {
                Err(RpcError::new2(MediaServerError::GatewayRpcError))
            }
        } else {
            Err(RpcError::new2(MediaServerError::InvalidConnId))
        }
    }
}

//TODO test
