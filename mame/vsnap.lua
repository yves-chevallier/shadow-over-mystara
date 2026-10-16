-- Instantanés de l'état vidéo, pour valider le moteur de rendu Rust contre MAME.
-- VSNAP="f1 f2 ..." : images à capturer ; VSNAP_DIR=dossier. Pour chaque image f, écrit dans
-- VSNAP_DIR/f/ : gfxram.bin, cps_a.bin, cps_b.bin, objram1.bin, objram2.bin, output.bin,
-- objbank.txt, et une capture PNG (dans le dossier -snapshot_directory de MAME).
-- VSNAP_GFX=1 : écrit aussi VSNAP_DIR/gfx.bin (région gfx décodée par MAME) une fois.

local machine = manager.machine
local shares = machine.memory.shares
local space = machine.devices[":maincpu"].spaces["program"]
local dir = os.getenv("VSNAP_DIR")
local wanted = {}
local prev = {}
for f in os.getenv("VSNAP"):gmatch("%d+") do wanted[tonumber(f)] = true; prev[tonumber(f) - 1] = true end

local objbank = 0
vsnap_bank = space:install_write_tap(0x8040e0, 0x8040e1, "vsnap_bank", function(offset, data, mask)
	objbank = data & 1
end)

-- Copie de palette au moment de l'écriture du registre CPS-A 0x0a (comme cps1_cps_a_w) :
-- 6 pages de 0x200 mots depuis la GFX RAM, filtrées par le registre CPS-B 0x30 au rendu.
local palette_latch = nil
vsnap_pal = space:install_write_tap(0x80410a, 0x80410b, "vsnap_pal", function(offset, data, mask)
	-- le tap passe avant l'écriture du registre : la base vient de la valeur écrite
	local s = shares[":gfxram"]
	local base = ((data * 256) & ~0x3ff) & 0x3ffff
	local parts = {}
	for i = 0, 6 * 0x200 - 1 do
		local w = s:read_u16((base + 2 * i) % s.size)
		parts[#parts + 1] = string.char(w >> 8, w & 0xff)
	end
	palette_latch = table.concat(parts)
end)

local function dump_share(name, path)
	local s = shares[name]
	local parts = {}
	for i = 0, s.size - 2, 2 do
		local w = s:read_u16(i)
		parts[#parts + 1] = string.char(w >> 8, w & 0xff)
	end
	local f = io.open(path, "wb")
	f:write(table.concat(parts))
	f:close()
end

if os.getenv("VSNAP_GFX") then
	local r = machine.memory.regions[":gfx"]
	local f = io.open(dir .. "/gfx.bin", "wb")
	local CH = 0x10000
	for base = 0, r.size - 1, CH do
		local parts = {}
		for i = base, base + CH - 1 do parts[#parts + 1] = string.char(r:read_u8(i)) end
		f:write(table.concat(parts))
	end
	f:close()
end

local frame = 0
vsnap_frame = emu.add_machine_frame_notifier(function()
	frame = frame + 1
	if prev[frame] then
		-- état objets de l'image précédente : MAME affiche la table verrouillée au vblank d'avant
		local d = string.format("%s/%d", dir, frame + 1)
		os.execute("mkdir -p " .. d)
		dump_share(":objram1", d .. "/prev_objram1.bin")
		dump_share(":objram2", d .. "/prev_objram2.bin")
		dump_share(":output", d .. "/prev_output.bin")
		local f = io.open(d .. "/prev_objbank.txt", "w"); f:write(tostring(objbank)); f:close()
	end
	if not wanted[frame] then return end
	local d = string.format("%s/%d", dir, frame)
	os.execute("mkdir -p " .. d)
	dump_share(":gfxram", d .. "/gfxram.bin")
	dump_share(":cps_a_regs", d .. "/cps_a.bin")
	dump_share(":cps_b_regs", d .. "/cps_b.bin")
	dump_share(":objram1", d .. "/objram1.bin")
	dump_share(":objram2", d .. "/objram2.bin")
	dump_share(":output", d .. "/output.bin")
	local f = io.open(d .. "/objbank.txt", "w"); f:write(tostring(objbank)); f:close()
	if palette_latch then
		local p = io.open(d .. "/palette_latch.bin", "wb"); p:write(palette_latch); p:close()
	end
	machine.video:snapshot()
	print("vsnap", frame)
end)
