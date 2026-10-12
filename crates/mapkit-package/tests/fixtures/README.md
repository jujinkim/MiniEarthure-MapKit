`indexed-v1-roads.mkregions` is the public MIT `examples/roads` synthetic source,
packed with the current v1 contract (four cells per region). It exercises the
current reader, declared decoder budget and geometry, with no historical loader.
It contains no external map data or private application source.
`stored-pipe-200-v1.memap` and `stored-pipe-300-v1.memap` are synthetic v1
packages produced before the 4 m authoring minimum. They prove stored geometry
loads unchanged while current recompilation rejects narrow source dimensions.
