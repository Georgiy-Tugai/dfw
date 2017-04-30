// Copyright 2017 Pit Kleyersburg <pitkley@googlemail.com>
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified or distributed
// except according to those terms.

//! # DFWRS - Docker Firewall Framework in Rust
//!
//! `dfwrs` is conceptually based on the [Docker Firewall Framework, `dfwfw`][dfwfw-github]. Its
//! goal is to make firewall administration with Docker simpler, but also more extensive by trying
//! to replace the Docker built-in firewall handling by direct interaction with iptables.
//!
//! This is accomplished by a flexible configuration which defines how the firewall should be built
//! up. While DFWRS is running, Docker container events will be monitored and the rules rebuilt
//! when necessary.
//!
//! See [DFWFW's README][dfwfw-readme] for more insight. Most of what you will read there will be
//! applicable to DFWRS.
//!
//! ## Configuration
//!
//! The general configuration happens across six categories:
//!
//! * `defaults`
//!
//!     This category defines global, default values to be used by DFWRS and the other categories.
//!
//! * `container_to_container`
//!
//!     This controls the communication between containers and across [Docker
//!     networks][docker-networks].
//!
//! * `container_to_wider_world`
//!
//!     This controls if and how containers may access the wider world, i.e. what they can
//!     communicate across the `OUTPUT` chain on the host.
//!
//! * `container_to_host`
//!
//!     To restrict or allow access to the host, this section is used.
//!
//! * `wider_world_to_container`
//!
//!     This controls how the wider world, i.e. whatever comes in through the `INPUT` chain on the
//!     host, can communicate with a container or a Docker network.
//!
//! * `container_dnat`
//!
//!     This category allows you to define specific rules for destination network address
//!     translation, even or especially across Docker networks.
//!
//! One category which DFWFW covers that is not (yet) implemented in DFWRS is
//! `container_internals`, that is configuring iptables rules within containers.
//!
//! See the [examples][examples] *(TODO)*, and the [configuration types][types.rs] for a detailed
//! description of every configuration section.
//!
//! ## Supported Docker versions
//!
//! At least Docker 1.9.0 is required, since we heavily rely on the Docker [networking
//! feature][docker-networks] which was introduced in 1.9.0.
//!
//! DFWRS has been successfully tested under the following Docker versions:
//!
//! * `17.03.1-ce`
//!
//! * `17.04.0-ce`
//!
//! It is planned to introduce some form of automated testing to cover as many Docker versions as
//! possible.
//!
//! ## Installation
//!
//! While you can use Cargo to install `dfwrs` as a binary, using the Docker image is the preferred
//! way to go, especially if you don't want to install Rust and Cargo on your host:
//!
//! ```console
//! $ docker pull pitkley/dfwrs:0.2
//! $ docker run -d \
//!       --name=dfwrs \
//!       -v /var/run/docker.sock:/var/run/docker.sock:ro \
//!       -v /path/to/your/config:/config \
//!       --net host --cap-add=NET_ADMIN \
//!       pitkley/dfwrs --config-path /config
//! ```
//!
//! This will download a lightweight image, coming in at under 6 MB, and subsequently run it using
//! your configuration.
//!
//! ## Motivation for this reimplementation
//!
//! I have reimplemented DFWFW in Rust for two reasons:
//!
//! 1. DFWFW has lost compatibility with the Docker API starting with release 17.04.0-ce.
//!
//!     This is very likely due to a change in Dockers web API regarding getting networks and their
//!     containers, see [this relevant issue][moby-issue-32686]. Now, it would almost certainly have
//!     been easier to fix this issue in DFWFW -- if not for me, maybe for the maintainer. I have
//!     [created an issue][dfwfw-issue-13] to give the DFWFW maintainer a heads-up.
//!
//! 2. But the main reason for this reimplementation was that I found a real-life project to tackle
//!    with Rust. This project allowed me to delve into quite a few different aspects and facets of
//!    Rust and especially its eco-system, amongst others:
//!
//!   * [`clap`][crates-clap], for parsing of command line arguments
//!   * [`chan`][crates-chan], for easy messaging and coordination between threads
//!   * [`error-chain`][crates-error-chain], for simplified application wide error handling
//!   * [Serde][crates-serde], for deserialization of the TOML configuration
//!   * [`slog`][crates-slog], for structured logging
//!
//!     Disregarding the obvious hair-pulling moments regarding ownership, borrowing and lifetimes,
//!     my experience with Rust, and its brillant eco-system has been an absolute pleasure.
//!
//! ## License
//!
//! DFWRS is licensed under either of
//!
//! * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
//!   http://www.apache.org/licenses/LICENSE-2.0)
//! * MIT license ([LICENSE-MIT](LICENSE-MIT) or
//!   http://opensource.org/licenses/MIT)
//!
//! at your option.
//!
//! ### Contribution
//!
//! Unless you explicitly state otherwise, any contribution intentionally submitted
//! for inclusion in DFWRS by you, as defined in the Apache-2.0 license, shall be
//! dual licensed as above, without any additional terms or conditions.
//!
//!
//! [crates-clap]: https://crates.io/crates/clap
//! [crates-chan]: https://crates.io/crates/chan
//! [crates-error-chain]: https://crates.io/crates/error-chain
//! [crates-serde]: https://crates.io/crates/serde
//! [crates-slog]: https://crates.io/crates/slog
//!
//! [dfwfw-github]: https://github.com/irsl/dfwfw
//! [dfwfw-issue-13]: https://github.com/irsl/dfwfw/issues/13
//! [dfwfw-readme]: https://github.com/irsl/dfwfw/blob/master/README.md
//!
//! [docker-networks]: https://docs.docker.com/engine/userguide/networking/
//!
//! [moby-issue-32686]: https://github.com/moby/moby/issues/32686
//!
//! [types.rs]: types/index.html

// Increase the compiler's recursion limit for the `error_chain` crate.
#![recursion_limit = "1024"]
#![deny(missing_docs)]

// Import external libraries
#[macro_use]
extern crate derive_builder;
#[macro_use]
extern crate error_chain;
extern crate glob;
extern crate iptables as ipt;
extern crate libc;
#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate shiplift;
#[macro_use]
extern crate slog;
extern crate slog_async;
extern crate slog_term;
extern crate time;
extern crate toml;
extern crate url;

// declare modules
pub mod errors;
pub mod iptables;
pub mod types;
pub mod util;

use errors::*;
use iptables::*;

use shiplift::Docker;
use shiplift::rep::{NetworkDetails, NetworkContainerDetails};
use shiplift::rep::Container;
use slog::Logger;
use std::collections::HashMap as Map;
use types::*;

const DFWRS_FORWARD_CHAIN: &'static str = "DFWRS_FORWARD";
const DFWRS_INPUT_CHAIN: &'static str = "DFWRS_INPUT";
const DFWRS_POSTROUTING_CHAIN: &'static str = "DFWRS_POSTROUTING";
const DFWRS_PREROUTING_CHAIN: &'static str = "DFWRS_PREROUTING";

#[derive(Debug, Clone, Default)]
struct Rule {
    pub source: Option<String>,
    pub destination: Option<String>,

    pub in_interface: Option<String>,
    pub out_interface: Option<String>,

    pub not_in_interface: bool,
    pub not_out_interface: bool,

    pub protocol: Option<String>,
    pub source_port: Option<String>,
    pub destination_port: Option<String>,

    pub filter: Option<String>,
    pub jump: Option<String>,
}

#[allow(dead_code)]
impl Rule {
    pub fn source(&mut self, value: String) -> &mut Self {
        let mut new = self;
        new.source = Some(value);
        new
    }

    pub fn destination(&mut self, value: String) -> &mut Self {
        let mut new = self;
        new.destination = Some(value);
        new
    }

    pub fn in_interface(&mut self, value: String) -> &mut Self {
        let mut new = self;
        new.in_interface = Some(value);
        new
    }

    pub fn out_interface(&mut self, value: String) -> &mut Self {
        let mut new = self;
        new.out_interface = Some(value);
        new
    }

    pub fn not_in_interface(&mut self, value: bool) -> &mut Self {
        let mut new = self;
        new.not_in_interface = value;
        new
    }

    pub fn not_out_interface(&mut self, value: bool) -> &mut Self {
        let mut new = self;
        new.not_out_interface = value;
        new
    }

    pub fn protocol(&mut self, value: String) -> &mut Self {
        let mut new = self;
        new.protocol = Some(value);
        new
    }

    pub fn source_port(&mut self, value: String) -> &mut Self {
        let mut new = self;
        new.source_port = Some(value);
        new
    }

    pub fn destination_port(&mut self, value: String) -> &mut Self {
        let mut new = self;
        new.destination_port = Some(value);
        new
    }

    pub fn filter(&mut self, value: String) -> &mut Self {
        let mut new = self;
        new.filter = Some(value);
        new
    }

    pub fn jump(&mut self, value: String) -> &mut Self {
        let mut new = self;
        new.jump = Some(value);
        new
    }

    pub fn build(&self) -> Result<String> {
        let mut args: Vec<String> = Vec::new();

        if let Some(ref source) = self.source {
            args.push("-s".to_owned());
            args.push(source.to_owned());
        }
        if let Some(ref destination) = self.destination {
            args.push("-d".to_owned());
            args.push(destination.to_owned());
        }
        if let Some(ref in_interface) = self.in_interface {
            if self.not_in_interface {
                args.push("!".to_owned());
            }
            args.push("-i".to_owned());
            args.push(in_interface.to_owned());
        }
        if let Some(ref out_interface) = self.out_interface {
            if self.not_out_interface {
                args.push("!".to_owned());
            }
            args.push("-o".to_owned());
            args.push(out_interface.to_owned());
        }

        // Bail if none of the above was initialized
        if args.len() <= 0 && self.filter.is_none() {
            bail!("one of `source`, `destination`, `in_interface`, `out_interface` \
                   or `filter` must be initialized");
        }

        if let Some(ref protocol) = self.protocol {
            args.push("-p".to_owned());
            args.push(protocol.to_owned());
        } else if self.source_port.is_some() || self.destination_port.is_some() {
            // Source and destination ports require that the protocol is set.
            // If it hasn't been specified explicitly, use "tcp" as default.
            args.push("-p".to_owned());
            args.push("tcp".to_owned());
        }

        if let Some(ref source_port) = self.source_port {
            args.push("--sport".to_owned());
            args.push(source_port.to_owned());
        }

        if let Some(ref destination_port) = self.destination_port {
            args.push("--dport".to_owned());
            args.push(destination_port.to_owned());
        }

        if let Some(ref jump) = self.jump {
            args.push("-j".to_owned());
            args.push(jump.to_owned());
        } else {
            bail!("`jump` must be initialized");
        }

        if let Some(ref filter) = self.filter {
            args.push(filter.to_owned());
        }

        Ok(args.join(" "))
    }
}

/// Enclosing struct to manage rule processing.
pub struct ProcessDFW<'a> {
    docker: &'a Docker,
    dfw: &'a DFW,
    ipt4: &'a IPTables,
    ipt6: &'a IPTables,
    container_map: Map<String, Container>,
    network_map: Map<String, NetworkDetails>,
    external_network_interfaces: Option<Vec<String>>,
    primary_external_network_interface: Option<String>,
    logger: Logger,
}

impl<'a> ProcessDFW<'a> {
    /// Create a new instance of `ProcessDFW` for rule processing.
    pub fn new(docker: &'a Docker,
               dfw: &'a DFW,
               ipt4: &'a IPTables,
               ipt6: &'a IPTables,
               logger: &'a Logger)
               -> Result<ProcessDFW<'a>> {
        let logger = logger.new(o!());

        let containers = docker.containers().list(&Default::default())?;
        debug!(logger, "Got list of containers";
               o!("containers" => format!("{:#?}", containers)));

        let container_map = get_container_map(&containers)?
            .ok_or_else(|| "no containers found")?;
        trace!(logger, "Got map of containers";
               o!("container_map" => format!("{:#?}", container_map)));

        let networks = docker.networks().list(&Default::default())?;
        debug!(logger, "Got list of networks";
               o!("networks" => format!("{:#?}", networks)));

        let network_map = get_network_map(&networks)?
            .ok_or_else(|| "no networks found")?;
        trace!(logger, "Got map of networks";
               o!("container_map" => format!("{:#?}", container_map)));

        let external_network_interfaces = dfw.defaults
            .as_ref()
            .and_then(|d| d.external_network_interfaces.as_ref())
            .map(Clone::clone);
        let primary_external_network_interface = external_network_interfaces
            .as_ref()
            .and_then(|v| v.get(0))
            .map(|s| s.to_owned());

        Ok(ProcessDFW {
               docker: docker,
               dfw: dfw,
               ipt4: ipt4,
               ipt6: ipt6,
               container_map: container_map,
               network_map: network_map,
               external_network_interfaces: external_network_interfaces,
               primary_external_network_interface: primary_external_network_interface,
               logger: logger,
           })
    }

    /// Start the processing using the configuration given at creation.
    pub fn process(&self) -> Result<()> {
        info!(self.logger, "Starting processing";
              o!("started_processing_at" => format!("{}", time::now().rfc3339())));

        create_and_flush_chain("filter", DFWRS_FORWARD_CHAIN, self.ipt4, self.ipt6)?;
        create_and_flush_chain("filter", DFWRS_INPUT_CHAIN, self.ipt4, self.ipt6)?;
        create_and_flush_chain("nat", DFWRS_PREROUTING_CHAIN, self.ipt4, self.ipt6)?;
        create_and_flush_chain("nat", DFWRS_POSTROUTING_CHAIN, self.ipt4, self.ipt6)?;
        debug!(self.logger, "Created and flushed chains");

        if let Some(ref init) = self.dfw.initialization {
            info!(self.logger, "Starting sub-processing";
                   o!("part" => "initialization"));
            self.process_initialization(init)?;
        }

        // Setup input and forward chain
        initialize_chain("filter", DFWRS_INPUT_CHAIN, self.ipt4, self.ipt6)?;
        self.ipt4
            .append("filter", "INPUT", &format!("-j {}", DFWRS_INPUT_CHAIN))?;
        initialize_chain("filter", DFWRS_FORWARD_CHAIN, self.ipt4, self.ipt6)?;
        self.ipt4
            .append("filter", "FORWARD", &format!("-j {}", DFWRS_FORWARD_CHAIN))?;
        // TODO: verify what is needed for ipt6
        debug!(self.logger, "Setup input and forward chains");

        // Setup pre- and postrouting
        self.ipt4
            .append("nat",
                    "PREROUTING",
                    &format!("-j {}", DFWRS_PREROUTING_CHAIN))?;
        self.ipt4
            .append("nat",
                    "POSTROUTING",
                    &format!("-j {}", DFWRS_POSTROUTING_CHAIN))?;
        // TODO: verify what is needed for ipt6
        debug!(self.logger, "Setup pre- and postrouting");

        if let Some(ref ctc) = self.dfw.container_to_container {
            info!(self.logger, "Starting sub-processing";
                  o!("part" => "container_to_container"));
            self.process_container_to_container(ctc)?;
        }
        if let Some(ref ctww) = self.dfw.container_to_wider_world {
            info!(self.logger, "Starting sub-processing";
                  o!("part" => "container_to_wider_world"));
            self.process_container_to_wider_world(ctww)?;
        }
        if let Some(ref cth) = self.dfw.container_to_host {
            info!(self.logger, "Starting sub-processing";
                  o!("part" => "container_to_host"));
            self.process_container_to_host(cth)?;
        }
        if let Some(ref wwtc) = self.dfw.wider_world_to_container {
            info!(self.logger, "Starting sub-processing";
                  o!("part" => "wider_world_to_container"));
            self.process_wider_world_to_container(wwtc)?;
        }
        if let Some(ref cd) = self.dfw.container_dnat {
            info!(self.logger, "Starting sub-processing";
                  o!("part" => "container_dnat"));
            self.process_container_dnat(cd)?;
        }

        if let Some(ref external_network_interfaces) = self.external_network_interfaces {
            for external_network_interface in external_network_interfaces {
                // Add accept rules for Docker bridge
                if let Some(bridge_network) = self.network_map.get("bridge") {
                    if let Some(bridge_name) =
                        bridge_network
                            .Options
                            .as_ref()
                            .ok_or("error")?
                            .get("com.docker.network.bridge.name") {
                        info!(self.logger, "Add ACCEPT rules for Docker bridge";
                              o!("docker_bridge" => bridge_name,
                                 "external_network_interface" => external_network_interface));

                        let rule_str = Rule::default()
                            .in_interface(bridge_name.to_owned())
                            .out_interface(external_network_interface.to_owned())
                            .jump("ACCEPT".to_owned())
                            .build()?;

                        trace!(self.logger, "Add forward rule for external network interface";
                               o!("external_network_interface" => external_network_interface,
                                  "rule" => rule_str.to_owned()));
                        self.ipt4
                            .append("filter", DFWRS_FORWARD_CHAIN, &rule_str)?;
                        // TODO: verify what is needed for ipt6

                        let rule_str = Rule::default()
                            .in_interface(bridge_name.to_owned())
                            .jump("ACCEPT".to_owned())
                            .build()?;

                        trace!(self.logger, "Add input rule for external network interface";
                               o!("external_network_interface" => external_network_interface,
                                  "rule" => &rule_str));
                        self.ipt4
                            .append("filter", DFWRS_INPUT_CHAIN, &rule_str)?;
                        // TODO: verify what is needed for ipt6
                    }
                }

                // Configure POSTROUTING
                info!(self.logger, "Configure postrouting for external network interface";
                      o!("external_network_interface" => external_network_interface));

                let rule_str = Rule::default()
                    .out_interface(external_network_interface.to_owned())
                    .jump("MASQUERADE".to_owned())
                    .build()?;

                trace!(self.logger, "Add post-routing rule for external network interface";
                       o!("external_network_interface" => external_network_interface,
                          "rule" => &rule_str));
                self.ipt4
                    .append("nat", DFWRS_POSTROUTING_CHAIN, &rule_str)?;
                // TODO: verify what is needed for ipt6
            }
        }

        // Set default policy for forward chain (defined by `container_to_container`)
        if let Some(ref ctc) = self.dfw.container_to_container {
            info!(self.logger, "Set default policy for forward chain";
                  o!("part" => "container_to_container",
                     "default_policy" => &ctc.default_policy));

            self.ipt4
                .append("filter",
                        DFWRS_FORWARD_CHAIN,
                        &format!("-j {}", ctc.default_policy))?;
            // TODO: verify what is needed for ipt6
        }

        Ok(())
    }

    fn process_initialization(&self, init: &Initialization) -> Result<()> {
        if let Some(ref v4) = init.v4 {
            info!(self.logger, "Process initialization rules";
                  o!("ip" => "v4"));
            for (table, rules) in v4.iter() {
                debug!(self.logger, "Process table";
                       o!("ip" => "v4",
                          "table" => table));
                for rule in rules {
                    debug!(self.logger, "Process rule";
                           o!("ip" => "v4",
                              "table" => table,
                              "rule" => rule));
                    let out = self.ipt4.execute(table, rule)?;
                    trace!(self.logger, "Rule executed";
                           o!("ip" => "v4",
                              "table" => table,
                              "rule" => rule,
                              "status" => out.status.code(),
                              "stdout" => String::from_utf8_lossy(&out.stdout).into_owned(),
                              "stderr" => String::from_utf8_lossy(&out.stderr).into_owned()))
                }
            }
        }

        if let Some(ref v6) = init.v6 {
            info!(self.logger, "Process initialization rules";
                  o!("ip" => "v4"));
            for (table, rules) in v6.iter() {
                debug!(self.logger, "Process table";
                       o!("ip" => "v4",
                          "table" => table));
                for rule in rules {
                    debug!(self.logger, "Process rule";
                           o!("ip" => "v4",
                              "table" => table,
                              "rule" => rule));
                    let out = self.ipt6.execute(table, rule)?;
                    trace!(self.logger, "Rule executed";
                           o!("ip" => "v4",
                              "table" => table,
                              "rule" => rule,
                              "status" => out.status.code(),
                              "stdout" => String::from_utf8_lossy(&out.stdout).into_owned(),
                              "stderr" => String::from_utf8_lossy(&out.stderr).into_owned()))
                }
            }
        }

        Ok(())
    }

    fn process_container_to_container(&self, ctc: &ContainerToContainer) -> Result<()> {
        if ctc.rules.is_some() {
            info!(self.logger, "Process rules";
                  o!("part" => "container_to_container"));
            self.process_ctc_rules(ctc.rules.as_ref().unwrap())?;
        }

        Ok(())
    }

    fn process_ctc_rules(&self, rules: &Vec<ContainerToContainerRule>) -> Result<()> {
        for rule in rules {
            info!(self.logger, "Process rule";
                  o!("part" => "container_to_container",
                     "rule" => format!("{:?}", rule)));
            let mut ipt_rule = Rule::default();

            let network = match self.network_map.get(&rule.network) {
                Some(network) => network,
                None => continue,
            };
            trace!(self.logger, "Got network";
                   o!("network_name" => &rule.network,
                      "network" => format!("{:?}", network)));

            let bridge_name = get_bridge_name(&network.Id)?;
            trace!(self.logger, "Got bridge name";
                   o!("network_name" => &network.Name,
                      "bridge_name" => &bridge_name));

            ipt_rule
                .in_interface(bridge_name.to_owned())
                .out_interface(bridge_name.to_owned());

            if let Some(ref src_container) = rule.src_container {
                let src_network = match get_network_for_container(&self.docker,
                                                                  &self.container_map,
                                                                  &src_container,
                                                                  &network.Id)? {
                    Some(src_network) => src_network,
                    None => continue,
                };
                trace!(self.logger, "Got source network";
                       o!("network_name" => &network.Name,
                          "src_network" => format!("{:?}", src_network)));

                let bridge_name = get_bridge_name(&network.Id)?;
                trace!(self.logger, "Got bridge name";
                       o!("network_name" => &network.Name,
                          "bridge_name" => &bridge_name));

                ipt_rule
                    .in_interface(bridge_name.to_owned())
                    .out_interface(bridge_name.to_owned())
                    .source(src_network
                                .IPv4Address
                                .split("/")
                                .next()
                                .unwrap()
                                .to_owned());
            }

            if let Some(ref dst_container) = rule.dst_container {
                let dst_network = match get_network_for_container(&self.docker,
                                                                  &self.container_map,
                                                                  &dst_container,
                                                                  &network.Id)? {
                    Some(dst_network) => dst_network,
                    None => continue,
                };
                trace!(self.logger, "Got destination network";
                       o!("network_name" => &network.Name,
                          "dst_network" => format!("{:?}", dst_network)));

                let bridge_name = get_bridge_name(&network.Id)?;
                trace!(self.logger, "Got bridge name";
                       o!("network_name" => &network.Name,
                          "bridge_name" => &bridge_name));

                ipt_rule
                    .out_interface(bridge_name.to_owned())
                    .destination(dst_network
                                     .IPv4Address
                                     .split("/")
                                     .next()
                                     .unwrap()
                                     .to_owned());
            }

            // Set jump
            ipt_rule.jump(rule.action.to_owned());

            let rule_str = ipt_rule.build()?;
            info!(self.logger, "Add forward rule";
                  o!("part" => "container_to_container",
                     "rule" => &rule_str));

            // Apply the rule
            self.ipt4
                .append("filter", DFWRS_FORWARD_CHAIN, &rule_str)?;
            // TODO: verify what is needed for ipt6
        }

        Ok(())
    }

    fn process_container_to_wider_world(&self, ctww: &ContainerToWiderWorld) -> Result<()> {
        // Rules
        if ctww.rules.is_some() {
            info!(self.logger, "Process rules";
                  o!("part" => "container_to_wider_world"));
            self.process_ctww_rules(ctww.rules.as_ref().unwrap())?;
        }

        // Default policy
        if let Some(ref external_network_interfaces) = self.external_network_interfaces {
            info!(self.logger, "Set default policy for external network interfaces";
                  o!("part" => "container_to_wider_world",
                     "external_network_interfaces" => format!("{:?}", external_network_interfaces),
                     "default_policy" => &ctww.default_policy));
            for external_network_interface in external_network_interfaces {
                trace!(self.logger, "Process default policy for external network interface";
                       o!("part" => "container_to_wider_world",
                          "external_network_interface" => external_network_interface,
                          "default_policy" => &ctww.default_policy));
                for (_, network) in &self.network_map {
                    let bridge_name = get_bridge_name(&network.Id)?;
                    trace!(self.logger, "Got bridge name";
                           o!("network_name" => &network.Name,
                              "bridge_name" => &bridge_name));

                    let rule = Rule::default()
                        .in_interface(bridge_name)
                        .out_interface(external_network_interface.to_owned())
                        .jump(ctww.default_policy.to_owned())
                        .build()?;

                    info!(self.logger, "Add forward rule for default policy";
                          o!("part" => "container_to_wider_world",
                             "external_network_interface" => external_network_interface,
                             "default_policy" => &ctww.default_policy,
                             "rule" => &rule));
                    self.ipt4.append("filter", DFWRS_FORWARD_CHAIN, &rule)?;
                    // TODO: verify what is needed for ipt6
                }
            }
        }

        Ok(())
    }

    fn process_ctww_rules(&self, rules: &Vec<ContainerToWiderWorldRule>) -> Result<()> {
        for rule in rules {
            info!(self.logger, "Process rule";
                  o!("part" => "container_to_wider_world",
                     "rule" => format!("{:?}", rule)));
            let mut ipt_rule = Rule::default();

            if let Some(ref network) = rule.network {
                if let Some(network) = self.network_map.get(network) {
                    let bridge_name = get_bridge_name(&network.Id)?;
                    trace!(self.logger, "Got bridge name";
                           o!("network_name" => &network.Name,
                              "bridge_name" => &bridge_name));

                    ipt_rule.in_interface(bridge_name.to_owned());

                    if let Some(ref src_container) = rule.src_container {
                        if let Some(src_network) =
                            get_network_for_container(&self.docker,
                                                      &self.container_map,
                                                      &src_container,
                                                      &network.Id)? {
                            trace!(self.logger, "Got source network";
                                   o!("network_name" => &network.Name,
                                      "src_network" => format!("{:?}", src_network)));

                            let bridge_name = get_bridge_name(&network.Id)?;
                            trace!(self.logger, "Got bridge name";
                                   o!("network_name" => &network.Name,
                                      "bridge_name" => &bridge_name));

                            ipt_rule
                                .in_interface(bridge_name.to_owned())
                                .source(src_network
                                            .IPv4Address
                                            .split("/")
                                            .next()
                                            .unwrap()
                                            .to_owned());
                        }
                    }
                }
            }

            if let Some(ref filter) = rule.filter {
                ipt_rule.filter(filter.to_owned());
            }

            ipt_rule.jump(rule.action.to_owned());

            // Try to build the rule without the out_interface defined to see if any of the other
            // mandatory fields has been populated.
            debug!(self.logger, "Build rule to verify contents";
                   o!("args" => format!("{:?}", ipt_rule)));
            ipt_rule.build()?; // TODO: maybe add a `verify` method to `Rule`

            if let Some(ref external_network_interface) = rule.external_network_interface {
                trace!(self.logger, "Rule has specific external network interface";
                       o!("external_network_interface" => external_network_interface));
                ipt_rule.out_interface(external_network_interface.to_owned());
            } else if let Some(ref primary_external_network_interface) =
                self.primary_external_network_interface {
                trace!(self.logger, "Rule uses primary external network interface";
                       o!("external_network_interface" => primary_external_network_interface));
                ipt_rule.out_interface(primary_external_network_interface.to_owned().to_owned());
            }

            let rule_str = ipt_rule.build()?;
            info!(self.logger, "Add forward rule";
                  o!("part" => "container_to_wider_world",
                     "rule" => &rule_str));

            // Apply the rule
            self.ipt4
                .append("filter", DFWRS_FORWARD_CHAIN, &rule_str)?;
            // TODO: verify what is needed for ipt6
        }

        Ok(())
    }

    fn process_container_to_host(&self, cth: &ContainerToHost) -> Result<()> {
        // Rules
        if cth.rules.is_some() {
            info!(self.logger, "Process rules";
                  o!("part" => "container_to_host"));
            self.process_cth_rules(cth.rules.as_ref().unwrap())?;
        }

        // Default policy
        for (_, network) in &self.network_map {
            let bridge_name = get_bridge_name(&network.Id)?;
            trace!(self.logger, "Got bridge name";
                   o!("network_name" => &network.Name,
                      "bridge_name" => &bridge_name));

            let rule = Rule::default()
                .in_interface(bridge_name)
                .jump(cth.default_policy.to_owned())
                .build()?;

            trace!(self.logger, "Add input rule for default policy";
                   o!("part" => "container_to_host",
                      "default_policy" => &cth.default_policy,
                      "rule" => &rule));
            self.ipt4.append("filter", DFWRS_INPUT_CHAIN, &rule)?;
            // TODO: verify what is needed for ipt6
        }

        Ok(())
    }

    fn process_cth_rules(&self, rules: &Vec<ContainerToHostRule>) -> Result<()> {
        for rule in rules {
            info!(self.logger, "Process rule";
                  o!("part" => "container_to_host",
                     "rule" => format!("{:?}", rule)));
            let mut ipt_rule = Rule::default();

            let network = match self.network_map.get(&rule.network) {
                Some(network) => network,
                None => continue,
            };
            trace!(self.logger, "Got network";
                   o!("network_name" => &network.Name,
                      "network" => format!("{:?}", network)));

            let bridge_name = get_bridge_name(&network.Id)?;
            trace!(self.logger, "Got bridge name";
                   o!("network_name" => &network.Name,
                      "bridge_name" => &bridge_name));

            ipt_rule.in_interface(bridge_name.to_owned());

            if let Some(ref src_container) = rule.src_container {
                if let Some(src_network) =
                    get_network_for_container(&self.docker,
                                              &self.container_map,
                                              &src_container,
                                              &network.Id)? {
                    trace!(self.logger, "Got source network";
                           o!("network_name" => &network.Name,
                              "src_network" => format!("{:?}", src_network)));
                    ipt_rule.source(src_network
                                        .IPv4Address
                                        .split("/")
                                        .next()
                                        .unwrap()
                                        .to_owned());
                }
            }

            if let Some(ref filter) = rule.filter {
                ipt_rule.filter(filter.to_owned());
            }

            ipt_rule.jump(rule.action.to_owned());

            // Try to build the rule without the out_interface defined to see if any of the other
            // mandatory fields has been populated.
            debug!(self.logger, "Build rule to verify contents";
                   o!("args" => format!("{:?}", ipt_rule)));
            ipt_rule.build()?; // TODO: maybe add a `verify` method to `Rule`

            let rule_str = ipt_rule.build()?;
            info!(self.logger, "Add input rule";
                  o!("part" => "container_to_host",
                     "rule" => &rule_str));

            // Apply the rule
            self.ipt4
                .append("filter", DFWRS_INPUT_CHAIN, &rule_str)?;
            // TODO: verify what is needed for ipt6
        }

        Ok(())
    }

    fn process_wider_world_to_container(&self, wwtc: &WiderWorldToContainer) -> Result<()> {
        if wwtc.rules.is_none() {
            trace!(self.logger, "No rules";
                   o!("part" => "wider_world_to_container"));
            return Ok(());
        }
        info!(self.logger, "Process rules";
              o!("part" => "wider_world_to_container"));

        let rules = wwtc.rules.as_ref().unwrap();

        for rule in rules {
            info!(self.logger, "Process rule";
                  o!("part" => "wider_world_to_container",
                     "rule" => format!("{:?}", rule)));
            for expose_port in &rule.expose_port {
                let mut ipt_forward_rule = Rule::default();
                let mut ipt_dnat_rule = Rule::default();

                let network = match self.network_map.get(&rule.network) {
                    Some(network) => network,
                    None => continue,
                };
                trace!(self.logger, "Got network";
                       o!("network_name" => &network.Name,
                          "network" => format!("{:?}", network)));

                let bridge_name = get_bridge_name(&network.Id)?;
                trace!(self.logger, "Got bridge name";
                       o!("network_name" => &network.Name,
                          "bridge_name" => &bridge_name));

                ipt_forward_rule.out_interface(bridge_name.to_owned());

                if let Some(dst_network) =
                    get_network_for_container(&self.docker,
                                              &self.container_map,
                                              &rule.dst_container,
                                              &network.Id)? {
                    trace!(self.logger, "Got destination network";
                           o!("network_name" => &network.Name,
                              "dst_network" => format!("{:?}", dst_network)));

                    ipt_forward_rule.destination(dst_network
                                                     .IPv4Address
                                                     .split("/")
                                                     .next()
                                                     .unwrap()
                                                     .to_owned());

                    let destination_port = match expose_port.container_port {
                        Some(destination_port) => destination_port.to_string(),
                        None => expose_port.host_port.to_string(),
                    };
                    ipt_forward_rule.destination_port(destination_port.to_owned());
                    ipt_dnat_rule.destination_port(destination_port.to_owned());
                    ipt_dnat_rule.filter(format!("--to-destination {}:{}",
                                                 dst_network
                                                     .IPv4Address
                                                     .split("/")
                                                     .next()
                                                     .unwrap(),
                                                 destination_port));
                } else {
                    // Network for container has to exist
                    continue;
                }

                // Set correct protocol
                ipt_forward_rule.protocol(expose_port.family.to_owned());
                ipt_dnat_rule.protocol(expose_port.family.to_owned());

                ipt_forward_rule.jump("ACCEPT".to_owned());
                ipt_dnat_rule.jump("DNAT".to_owned());

                // Try to build the rule without the out_interface defined to see if any of the
                // other mandatory fields has been populated.
                debug!(self.logger, "Build rule to verify contents";
                       o!("args" => format!("{:?}", ipt_forward_rule)));
                ipt_forward_rule.build()?; // TODO: maybe add a `verify` method to `Rule`
                debug!(self.logger, "Build rule to verify contents";
                       o!("args" => format!("{:?}", ipt_dnat_rule)));
                ipt_dnat_rule.build()?; // TODO: maybe add a `verify` method to `Rule`

                if let Some(ref external_network_interface) = rule.external_network_interface {
                    trace!(self.logger, "Rule has specific external network interface";
                           o!("external_network_interface" => external_network_interface));

                    ipt_forward_rule.in_interface(external_network_interface.to_owned());
                    ipt_dnat_rule.in_interface(external_network_interface.to_owned());
                } else if let Some(ref primary_external_network_interface) =
                    self.primary_external_network_interface {
                    trace!(self.logger, "Rule uses primary external network interface";
                           o!("external_network_interface" => primary_external_network_interface));

                    ipt_forward_rule.in_interface(primary_external_network_interface
                                                      .to_owned()
                                                      .to_owned());
                    ipt_dnat_rule.in_interface(primary_external_network_interface
                                                   .to_owned()
                                                   .to_owned());
                } else {
                    // The DNAT rule requires the external interface
                    continue;
                }

                let forward_rule_str = ipt_forward_rule.build()?;
                info!(self.logger, "Add forward rule";
                      o!("part" => "wider_world_to_container",
                         "rule" => &forward_rule_str));
                let dnat_rule_str = ipt_dnat_rule.build()?;
                info!(self.logger, "Add DNAT rule";
                      o!("part" => "wider_world_to_container",
                         "rule" => &dnat_rule_str));

                // Apply the rule
                self.ipt4
                    .append("filter", DFWRS_FORWARD_CHAIN, &forward_rule_str)?;
                self.ipt4
                    .append("nat", DFWRS_PREROUTING_CHAIN, &dnat_rule_str)?;
                // TODO: verify what is needed for ipt6

            }
        }
        Ok(())
    }

    fn process_container_dnat(&self, cd: &ContainerDNAT) -> Result<()> {
        if cd.rules.is_none() {
            trace!(self.logger, "No rules";
                   o!("part" => "container_dnat"));
            return Ok(());
        }
        info!(self.logger, "Process rules";
              o!("part" => "container_dnat"));

        let rules = cd.rules.as_ref().unwrap();

        for rule in rules {
            info!(self.logger, "Process rule";
                  o!("part" => "container_dnat",
                     "rule" => format!("{:?}", rule)));
            for expose_port in &rule.expose_port {
                let mut ipt_rule = Rule::default();

                if let Some(ref network) = rule.src_network {
                    if let Some(network) = self.network_map.get(network) {
                        trace!(self.logger, "Got network";
                               o!("network_name" => &network.Name,
                                  "network" => format!("{:?}", network)));

                        let bridge_name = get_bridge_name(&network.Id)?;
                        trace!(self.logger, "Got bridge name";
                               o!("network_name" => &network.Name,
                                  "bridge_name" => &bridge_name));

                        ipt_rule.in_interface(bridge_name.to_owned());

                        if let Some(ref src_container) = rule.src_container {
                            if let Some(src_network) =
                                get_network_for_container(&self.docker,
                                                          &self.container_map,
                                                          &src_container,
                                                          &network.Id)? {
                                trace!(self.logger, "Got source network";
                                       o!("network_name" => &network.Name,
                                          "src_network" => format!("{:?}", src_network)));

                                let bridge_name = get_bridge_name(&network.Id)?;
                                trace!(self.logger, "Got bridge name";
                                       o!("network_name" => &network.Name,
                                          "bridge_name" => &bridge_name));

                                ipt_rule
                                    .in_interface(bridge_name.to_owned())
                                    .source(src_network
                                                .IPv4Address
                                                .split("/")
                                                .next()
                                                .unwrap()
                                                .to_owned());
                            }
                        }
                    }
                }

                let network = match self.network_map.get(&rule.dst_network) {
                    Some(network) => network,
                    None => continue,
                };
                let dst_network = match get_network_for_container(&self.docker,
                                                                  &self.container_map,
                                                                  &rule.dst_container,
                                                                  &network.Id)? {
                    Some(dst_network) => dst_network,
                    None => continue,
                };
                let destination_port = match expose_port.container_port {
                    Some(destination_port) => destination_port.to_string(),
                    None => expose_port.host_port.to_string(),
                };
                ipt_rule.destination_port(destination_port.to_owned());
                ipt_rule.filter(format!("--to-destination {}:{}",
                                        dst_network.IPv4Address.split("/").next().unwrap(),
                                        destination_port));

                ipt_rule.jump("DNAT".to_owned());

                // Try to build the rule without the out_interface defined to see if any of the
                // other mandatory fields has been populated.
                debug!(self.logger, "Build rule to verify contents";
                       o!("args" => format!("{:?}", ipt_rule)));
                ipt_rule.build()?; // TODO: maybe add a `verify` method to `Rule`

                if ipt_rule.out_interface.is_none() {
                    if let Some(ref primary_external_network_interface) =
                        self.primary_external_network_interface {
                        trace!(self.logger, "Set primary external network interface";
                               o!("external_network_interface"
                                  => primary_external_network_interface));

                        ipt_rule
                            .in_interface(primary_external_network_interface.to_owned().to_owned())
                            .not_in_interface(true);
                    } else {
                        // We need to specify a external network interface.
                        // If it is not defined, skip the rule.
                        continue;
                    }
                }

                let rule_str = ipt_rule.build()?;
                info!(self.logger, "Add prerouting rule";
                      o!("part" => "container_dnat",
                         "rule" => &rule_str));

                // Apply the rule
                self.ipt4
                    .append("nat", DFWRS_PREROUTING_CHAIN, &rule_str)?;
                // TODO: verify what is needed for ipt6
            }
        }

        Ok(())
    }
}




fn create_and_flush_chain(table: &str,
                          chain: &str,
                          ipt4: &IPTables,
                          ipt6: &IPTables)
                          -> Result<()> {
    // Create and flush CTC chain
    ipt4.new_chain(table, chain)?;
    ipt6.new_chain(table, chain)?;
    ipt4.flush_chain(table, chain)?;
    ipt6.flush_chain(table, chain)?;

    Ok(())
}

fn initialize_chain(table: &str, chain: &str, ipt4: &IPTables, ipt6: &IPTables) -> Result<()> {
    // Drop INVALID, accept RELATED/ESTABLISHED
    ipt4.append(table, chain, "-m state --state INVALID -j DROP")?;
    ipt6.append(table, chain, "-m state --state INVALID -j DROP")?;
    ipt4.append(table,
                chain,
                "-m state --state RELATED,ESTABLISHED -j ACCEPT")?;
    ipt6.append(table,
                chain,
                "-m state --state RELATED,ESTABLISHED -j ACCEPT")?;

    Ok(())
}

fn get_bridge_name(network_id: &str) -> Result<String> {
    if network_id.len() < 12 {
        bail!("network has to be longer than 12 characters");
    }
    Ok(format!("br-{}", &network_id[..12]))
}

fn get_network_for_container(docker: &Docker,
                             container_map: &Map<String, Container>,
                             container_name: &str,
                             network_id: &str)
                             -> Result<Option<NetworkContainerDetails>> {
    Ok(match container_map.get(container_name) {
           Some(container) => {
               match docker
                         .networks()
                         .get(network_id)
                         .inspect()?
                         .Containers
                         .get(&container.Id) {
                   Some(network) => Some(network.clone()),
                   None => None,
               }
           }
           None => None,
       })
}

fn get_container_map(containers: &Vec<Container>) -> Result<Option<Map<String, Container>>> {
    let mut container_map: Map<String, Container> = Map::new();
    for container in containers {
        for name in &container.Names {
            container_map.insert(name.clone().trim_left_matches("/").to_owned(),
                                 container.clone());
        }
    }

    if container_map.is_empty() {
        Ok(None)
    } else {
        Ok(Some(container_map))
    }
}

fn get_network_map(networks: &Vec<NetworkDetails>) -> Result<Option<Map<String, NetworkDetails>>> {
    let mut network_map: Map<String, NetworkDetails> = Map::new();
    for network in networks {
        network_map.insert(network.Name.clone(), network.clone());
    }

    if network_map.is_empty() {
        Ok(None)
    } else {
        Ok(Some(network_map))
    }
}