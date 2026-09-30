--[[
	The contract with the core, in one place.

	These constants MUST MATCH the core. Their counterparts:
	  VERSION    <-> packages/core-engine/Cargo.toml  version
	  PROTOCOL   <-> project.rs  PROTOCOL_VERSION
	  PORT_START <-> project.rs  DEFAULT_PORT
	  PORT_RANGE <-> project.rs  PORT_SCAN_SPAN

	The plugin's own version lives here too, so the panel and the handshake cannot
	report two different numbers.
]]

return {
	VERSION = "0.1.7",
	PROTOCOL = 1,
	PORT_START = 8080,
	PORT_RANGE = 10,

	-- Studio refuses to POST more than 1024 KB ("Post data too large"). Bigger messages
	-- are split below this, leaving room for the message around the split part.
	MAX_POST_BYTES = 900 * 1024,
}
