use std::{convert::TryInto, io};
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use super::common::Address;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Invalid version: {0}")]
    InvalidVersion(u8),
    #[error("Too many methods")]
    TooManyMethods,
    #[error("Invalid handshake")]
    InvalidHandshake,
    #[error("Invalid command: {0}")]
    InvalidCommand(u8),
    #[error("Invalid command reply: {0}")]
    InvalidCommandReply(u8),
    #[error("Command reply with error: {0:?}")]
    CommandReply(CommandReply),
    #[error("IO error: {0:?}")]
    Io(#[from] io::Error),
}
pub type Result<T, E = Error> = ::std::result::Result<T, E>;

#[derive(Debug)]
pub enum Version {
    V5,
}
impl Version {
    pub async fn read(mut reader: impl AsyncRead + Unpin) -> Result<Version> {
        let version = &mut [0u8];
        reader.read_exact(version).await?;
        match version[0] {
            5 => Ok(Version::V5),
            other => Err(Error::InvalidVersion(other)),
        }
    }
    pub async fn write(&self, mut writer: impl AsyncWrite + Unpin) -> Result<()> {
        let v = match self {
            Version::V5 => 5u8,
        };
        writer.write_all(&[v]).await?;
        Ok(())
    }
}

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub enum AuthMethod {
    Noauth,
    Gssapi,
    UsernamePassword,
    NoAcceptableMethod,
    Other(u8),
}

impl From<u8> for AuthMethod {
    fn from(n: u8) -> Self {
        match n {
            0x00 => AuthMethod::Noauth,
            0x01 => AuthMethod::Gssapi,
            0x02 => AuthMethod::UsernamePassword,
            0xff => AuthMethod::NoAcceptableMethod,
            other => AuthMethod::Other(other),
        }
    }
}

impl Into<u8> for AuthMethod {
    fn into(self) -> u8 {
        match self {
            AuthMethod::Noauth => 0x00,
            AuthMethod::Gssapi => 0x01,
            AuthMethod::UsernamePassword => 0x02,
            AuthMethod::NoAcceptableMethod => 0xff,
            AuthMethod::Other(other) => other,
        }
    }
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct AuthRequest(Vec<AuthMethod>);

impl AuthRequest {
    pub fn new(methods: impl Into<Vec<AuthMethod>>) -> AuthRequest {
        AuthRequest(methods.into())
    }
    pub async fn read(mut reader: impl AsyncRead + Unpin) -> Result<AuthRequest> {
        let count = &mut [0u8];
        reader.read_exact(count).await?;
        let mut methods = vec![0u8; count[0] as usize];
        reader.read_exact(&mut methods).await?;

        Ok(AuthRequest(methods.into_iter().map(Into::into).collect()))
    }
    pub async fn write(&self, mut writer: impl AsyncWrite + Unpin) -> Result<()> {
        let count = self.0.len();
        if count > 255 {
            return Err(Error::TooManyMethods);
        }

        writer.write_all(&[count as u8]).await?;
        writer
            .write_all(
                &self
                    .0
                    .iter()
                    .map(|i| Into::<u8>::into(*i))
                    .collect::<Vec<_>>(),
            )
            .await?;

        Ok(())
    }
    pub fn select_from(&self, auth: &[AuthMethod]) -> AuthMethod {
        self.0
            .iter()
            .enumerate()
            .find(|(_, m)| auth.contains(*m))
            .map(|(v, _)| AuthMethod::from(v as u8))
            .unwrap_or(AuthMethod::NoAcceptableMethod)
    }
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct AuthResponse(AuthMethod);

impl AuthResponse {
    pub fn new(method: AuthMethod) -> AuthResponse {
        AuthResponse(method)
    }
    pub async fn read(mut reader: impl AsyncRead + Unpin) -> Result<AuthResponse> {
        let method = &mut [0u8];
        reader.read_exact(method).await?;
        Ok(AuthResponse(method[0].into()))
    }
    pub async fn write(&self, mut writer: impl AsyncWrite + Unpin) -> Result<()> {
        writer.write_all(&[self.0.into()]).await?;
        Ok(())
    }
    pub fn method(&self) -> AuthMethod {
        self.0
    }
}

#[derive(Debug)]
pub enum Command {
    Connect,
    Bind,
    UdpAssociate,
}
#[derive(Debug)]
pub struct CommandRequest {
    pub command: Command,
    pub address: Address,
}

impl CommandRequest {
    pub fn connect(address: Address) -> CommandRequest {
        CommandRequest {
            command: Command::Connect,
            address,
        }
    }
    pub fn udp_associate(address: Address) -> CommandRequest {
        CommandRequest {
            command: Command::UdpAssociate,
            address,
        }
    }
    pub async fn read(mut reader: impl AsyncRead + Unpin) -> Result<CommandRequest> {
        let buf = &mut [0u8; 3];
        reader.read_exact(buf).await?;
        if buf[0] != 5 {
            return Err(Error::InvalidVersion(buf[0]));
        }
        if buf[2] != 0 {
            return Err(Error::InvalidHandshake);
        }
        let cmd = match buf[1] {
            1 => Command::Connect,
            2 => Command::Bind,
            3 => Command::UdpAssociate,
            _ => return Err(Error::InvalidCommand(buf[1])),
        };

        let address = Address::read(reader).await?;

        Ok(CommandRequest {
            command: cmd,
            address,
        })
    }
    pub async fn write(&self, mut writer: impl AsyncWrite + Unpin) -> Result<()> {
        let cmd = match self.command {
            Command::Connect => 1u8,
            Command::Bind => 2,
            Command::UdpAssociate => 3,
        };
        writer.write_all(&[0x05, cmd, 0x00]).await?;
        self.address.write(writer).await?;
        Ok(())
    }
}

#[derive(Debug, PartialEq, PartialOrd)]
pub enum CommandReply {
    Succeeded,
    GeneralSocksServerFailure,
    ConnectionNotAllowedByRuleset,
    NetworkUnreachable,
    HostUnreachable,
    ConnectionRefused,
    TtlExpired,
    CommandNotSupported,
    AddressTypeNotSupported,
}

impl CommandReply {
    pub fn from_u8(n: u8) -> Result<CommandReply> {
        Ok(match n {
            0 => CommandReply::Succeeded,
            1 => CommandReply::GeneralSocksServerFailure,
            2 => CommandReply::ConnectionNotAllowedByRuleset,
            3 => CommandReply::NetworkUnreachable,
            4 => CommandReply::HostUnreachable,
            5 => CommandReply::ConnectionRefused,
            6 => CommandReply::TtlExpired,
            7 => CommandReply::CommandNotSupported,
            8 => CommandReply::AddressTypeNotSupported,
            _ => return Err(Error::InvalidCommandReply(n)),
        })
    }
    pub fn to_u8(&self) -> u8 {
        match self {
            CommandReply::Succeeded => 0,
            CommandReply::GeneralSocksServerFailure => 1,
            CommandReply::ConnectionNotAllowedByRuleset => 2,
            CommandReply::NetworkUnreachable => 3,
            CommandReply::HostUnreachable => 4,
            CommandReply::ConnectionRefused => 5,
            CommandReply::TtlExpired => 6,
            CommandReply::CommandNotSupported => 7,
            CommandReply::AddressTypeNotSupported => 8,
        }
    }
}

#[derive(Debug)]
pub struct CommandResponse {
    pub reply: CommandReply,
    pub address: Address,
}

impl CommandResponse {
    pub fn success(address: Address) -> CommandResponse {
        CommandResponse {
            reply: CommandReply::Succeeded,
            address,
        }
    }
    pub fn reply_error(reply: CommandReply) -> CommandResponse {
        CommandResponse {
            reply,
            address: Default::default(),
        }
    }
    pub fn error(e: impl TryInto<io::Error>) -> CommandResponse {
        match e.try_into() {
            Ok(v) => {
                use io::ErrorKind;
                let reply = match v.kind() {
                    ErrorKind::ConnectionRefused => CommandReply::ConnectionRefused,
                    _ => CommandReply::GeneralSocksServerFailure,
                };
                CommandResponse {
                    reply,
                    address: Default::default(),
                }
            }
            Err(_) => CommandResponse {
                reply: CommandReply::GeneralSocksServerFailure,
                address: Default::default(),
            },
        }
    }
    pub async fn read(mut reader: impl AsyncRead + Unpin) -> Result<CommandResponse> {
        let buf = &mut [0u8; 3];
        reader.read_exact(buf).await?;
        if buf[0] != 5 {
            return Err(Error::InvalidVersion(buf[0]));
        }
        if buf[2] != 0 {
            return Err(Error::InvalidHandshake);
        }
        let reply = CommandReply::from_u8(buf[1])?;

        let address = Address::read(reader).await?;

        if reply != CommandReply::Succeeded {
            return Err(Error::CommandReply(reply));
        }

        Ok(CommandResponse { reply, address })
    }
    pub async fn write(&self, mut writer: impl AsyncWrite + Unpin) -> Result<()> {
        writer.write_all(&[0x05, self.reply.to_u8(), 0x00]).await?;
        self.address.write(writer).await?;
        Ok(())
    }
}
