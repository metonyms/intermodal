use crate::common::*;

pub(crate) struct TorrentSummary {
  metainfo: Metainfo,
  infohash: sha1::Digest,
  size: Bytes,
}

impl TorrentSummary {
  fn new(bytes: &[u8], metainfo: Metainfo) -> Result<Self, Error> {
    let value = bencode::Value::decode(&bytes).unwrap();

    let infohash = if let bencode::Value::Dict(items) = value {
      let info = items
        .iter()
        .find(|(key, _value)| key == b"info")
        .unwrap()
        .1
        .encode();
      Sha1::from(info).digest()
    } else {
      unreachable!()
    };

    Ok(Self {
      size: Bytes::from(bytes.len().into_u64()),
      infohash,
      metainfo,
    })
  }

  pub(crate) fn from_metainfo(metainfo: Metainfo) -> Result<Self, Error> {
    let bytes = metainfo.serialize()?;
    Self::new(&bytes, metainfo)
  }

  pub(crate) fn load(path: &Path) -> Result<Self, Error> {
    let bytes = fs::read(path).context(error::Filesystem { path })?;

    let metainfo = Metainfo::deserialize(path, &bytes)?;

    Self::new(&bytes, metainfo)
  }

  pub(crate) fn write(&self, env: &mut Env) -> Result<(), Error> {
    let table = self.table();

    if env.out_is_term() {
      let out_style = env.out_style();
      table
        .write_human_readable(&mut env.out, out_style)
        .context(error::Stdout)?;
    } else {
      table
        .write_tab_delimited(&mut env.out)
        .context(error::Stdout)?;
    }

    Ok(())
  }

  fn table(&self) -> Table {
    let mut table = Table::new();

    table.row("Name", &self.metainfo.info.name);

    if let Some(comment) = &self.metainfo.comment {
      table.row("Comment", comment);
    }

    if let Some(creation_date) = self.metainfo.creation_date {
      #[allow(clippy::as_conversions)]
      table.row(
        "Created",
        Utc.timestamp(
          creation_date
            .min(i64::max_value() as u64)
            .try_into()
            .unwrap(),
          0,
        ),
      );
    }

    if let Some(source) = &self.metainfo.info.source {
      table.row("Source", source);
    }

    table.row("Info Hash", self.infohash);

    table.size("Torrent Size", self.size);

    table.size("Content Size", self.metainfo.info.mode.total_size());

    table.row(
      "Private",
      if self.metainfo.info.private.unwrap_or(0) == 1 {
        "yes"
      } else {
        "no"
      },
    );

    match &self.metainfo.announce_list {
      Some(tiers) => {
        let mut value = Vec::new();

        if !tiers
          .iter()
          .any(|tier| tier.contains(&self.metainfo.announce))
        {
          value.push(("Main".to_owned(), vec![self.metainfo.announce.clone()]));
        }

        for (i, tier) in tiers.iter().enumerate() {
          value.push((format!("Tier {}", i + 1), tier.clone()));
        }

        table.tiers("Trackers", value);
      }
      None => table.row("Tracker", &self.metainfo.announce),
    }

    table.size("Piece Size", Bytes::from(self.metainfo.info.piece_length));

    table.row("Piece Count", self.metainfo.info.pieces.len() / 20);

    table.row(
      "File Count",
      match &self.metainfo.info.mode {
        Mode::Single { .. } => 1,
        Mode::Multiple { files } => files.len(),
      },
    );

    table
  }
}