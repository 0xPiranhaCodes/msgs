use crate::{
    crypto::derive_room_keys,
    domain::{Identity, InstanceId},
    net::{
        discovery::parse_seeds,
        mesh::{MeshConfig, MeshHandle},
    },
    tui::{self, model::Model, terminal::install_panic_hook},
};
use anyhow::{anyhow, Context, Result};
use clap::Parser;
use rand::RngCore;
use std::process::Command;
use zeroize::Zeroizing;

#[derive(Clone, Debug, Parser, PartialEq, Eq)]
#[command(name = "msgs", about = "Ephemeral encrypted chat over your VPN")]
pub struct Options {
    #[arg(long = "peer", value_name = "VPN-IP[:PORT]")]
    pub peers: Vec<String>,

    #[arg(long, default_value_t = 45_873)]
    pub port: u16,
}

pub fn load_git_identity<F>(mut read: F) -> Result<Identity>
where
    F: FnMut(&str) -> Result<String>,
{
    let name = read_git_value(&mut read, "user.name")?;
    let email = read_git_value(&mut read, "user.email")?;
    Ok(Identity { name, email })
}

fn read_git_value<F>(read: &mut F, key: &str) -> Result<String>
where
    F: FnMut(&str) -> Result<String>,
{
    let value =
        read(key).with_context(|| format!("set it with `git config --global {key} <value>`"))?;
    let value = value.trim();
    if value.is_empty() {
        return Err(anyhow!(
            "set the missing value with `git config --global {key} <value>`"
        ));
    }
    Ok(value.to_owned())
}

pub fn load_system_git_identity() -> Result<Identity> {
    load_git_identity(|key| {
        let output = Command::new("git")
            .args(["config", "--global", "--get", key])
            .output()
            .context("run git config")?;
        if !output.status.success() {
            return Err(anyhow!("Git configuration value is missing"));
        }
        String::from_utf8(output.stdout).context("Git configuration is not UTF-8")
    })
}

pub async fn run(options: Options) -> Result<()> {
    let identity = load_system_git_identity()?;
    let seeds = parse_seeds(&options.peers)?;
    let passphrase =
        rpassword::prompt_password("Room passphrase: ").context("read room passphrase")?;
    let keys = derive_room_keys(Zeroizing::new(passphrase.into_bytes()))?;
    let mut id = [0_u8; 16];
    rand::rngs::OsRng.fill_bytes(&mut id);
    let mesh = MeshHandle::spawn(MeshConfig {
        identity: identity.clone(),
        id: InstanceId(id),
        port: options.port,
        keys,
        seeds,
        multicast: true,
    })
    .await?;
    install_panic_hook();
    tui::run(Model::new(identity), mesh).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_repeatable_peers_and_default_port() {
        let options =
            Options::try_parse_from(["msgs", "--peer", "10.0.0.4", "--peer=10.0.0.5:5000"])
                .unwrap();
        assert_eq!(options.port, 45_873);
        assert_eq!(options.peers, ["10.0.0.4", "10.0.0.5:5000"]);
    }

    #[test]
    fn rejects_positional_arguments() {
        assert!(Options::try_parse_from(["msgs", "room"]).is_err());
    }

    #[test]
    fn loads_and_trims_git_identity() {
        let identity = load_git_identity(|key| match key {
            "user.name" => Ok(" Ada Lovelace\n".into()),
            "user.email" => Ok(" ada@example.com \n".into()),
            _ => unreachable!(),
        })
        .unwrap();
        assert_eq!(identity.name, "Ada Lovelace");
        assert_eq!(identity.email, "ada@example.com");
    }

    #[test]
    fn missing_git_name_has_actionable_error() {
        let error = load_git_identity(|key| {
            if key == "user.name" {
                Err(anyhow::anyhow!("not configured"))
            } else {
                Ok("ada@example.com".into())
            }
        })
        .unwrap_err();
        assert!(error.to_string().contains("git config --global user.name"));
    }
}
