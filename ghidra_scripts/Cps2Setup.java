// Prépare un programme CPS2 importé depuis l'image d'opcodes déchiffrés (prog_dec.bin) :
// carte mémoire, labels d'E/S, vecteurs, puis fusion itérative opcodes déchiffrés / données en clair.
//
// Sur CPS2 seules les lectures d'opcodes sont déchiffrées, les données sont lues en clair.
// Ghidra n'a qu'une vue mémoire : on y place les octets déchiffrés là où il y a des
// instructions et les octets en clair ailleurs, et on itère jusqu'à ce que l'ensemble
// des points d'entrée de code ne change plus.
//
// Sur CPS2 sont déchiffrés les fetchs d'instructions ET les lectures relatives au PC.
//
// Arguments : <prog_plain.bin> <plage_chiffrée_fin> <sortie_merged.bin> [coverage.bin]
// coverage.bin (mame/coverage.lua) : un octet par mot, 1 = lu en espace programme pendant l'exécution.
// Ces mots sont forcément déchiffrés ; chaque début de plage couverte sert aussi de point d'entrée,
// désassemblé sans sortir des zones couvertes.
// @category CPS2

import java.io.File;
import java.nio.file.Files;
import java.util.*;

import ghidra.app.cmd.disassemble.DisassembleCommand;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.*;
import ghidra.program.model.lang.Register;
import ghidra.program.model.listing.*;
import ghidra.program.model.mem.*;
import ghidra.program.model.symbol.*;
import ghidra.program.model.data.*;

public class Cps2Setup extends GhidraScript {

	private byte[] plain;
	private byte[] dec;
	private int encEnd;

	@Override
	protected void run() throws Exception {
		String[] args = getScriptArgs();
		plain = Files.readAllBytes(new File(args[0]).toPath());
		encEnd = Integer.decode(args[1]);
		File outMerged = new File(args[2]);
		if (args.length > 3) loadCoverage(new File(args[3]));

		Memory mem = currentProgram.getMemory();
		dec = new byte[plain.length];
		mem.getBytes(toAddr(0), dec);

		setupMemoryMap(mem);
		setupIoLabels();

		// Points d'entrée : reset (vecteur lu via l'espace opcodes) + vecteurs d'exception (lus en clair).
		TreeSet<Long> entries = new TreeSet<>();
		long resetPc = be32(dec, 4);
		entries.add(resetPc);
		label(resetPc, "reset");
		String[] names = new String[64];
		names[2] = "exc_bus_error"; names[3] = "exc_address_error"; names[4] = "exc_illegal";
		names[5] = "exc_zero_divide"; names[6] = "exc_chk"; names[7] = "exc_trapv";
		names[8] = "exc_privilege"; names[9] = "exc_trace"; names[10] = "exc_line_a";
		names[11] = "exc_line_f"; names[24] = "irq_spurious";
		for (int i = 1; i <= 7; i++) names[24 + i] = "irq" + i;
		names[26] = "irq2_vblank";
		names[28] = "irq4_raster";
		for (int i = 0; i < 16; i++) names[32 + i] = "trap" + i;
		for (int v = 2; v < 64; v++) {
			long t = be32(plain, v * 4);
			if (t < plain.length && (t & 1) == 0) {
				entries.add(t);
				if (names[v] != null) label(t, names[v]);
			}
		}

		// Les fonctions de l'ordonnanceur (yield : restaure SP puis rts) passent pour non-retournantes
		// et Ghidra efface alors le code qui suit leurs appels.
		setAnalysisOption(currentProgram, "Non-Returning Functions - Discovered", "false");

		Set<Long> code = new HashSet<>();
		for (int pass = 1; pass <= 12; pass++) {
			monitor.checkCancelled();
			// a) mémoire entièrement déchiffrée, désassemblage par flot seulement depuis les entrées connues
			clearAll();
			writeRange(mem, dec, 0, encEnd);
			pcData.clear();
			Set<Long> done = new HashSet<>();
			while (true) {
				for (long e : entries) if (done.add(e)) disassemble(toAddr(e));
				for (long e : covEntries) {
					if (!done.add(-e - 1)) continue;
					if (currentProgram.getListing().getInstructionContaining(toAddr(e)) != null) continue;
					new DisassembleCommand(toAddr(e), covRestrict, true).applyTo(currentProgram, monitor);
				}
				Set<Long> found = resolvePcTables();
				found.removeAll(entries);
				if (found.isEmpty()) break;
				entries.addAll(found);
			}
			code = instructionBytes(encEnd);
			code.addAll(pcData);
			code.addAll(covBytes);

			// b) mémoire fusionnée, analyse complète (tables de sauts, pointeurs, fonctions)
			byte[] merged = merge(code);
			writeRange(mem, merged, 0, encEnd);
			analyzeAll(currentProgram);

			// c) nouvelles cibles de code découvertes dans la zone chiffrée hors du code connu
			Set<Long> fresh = new TreeSet<>();
			for (Function f : currentProgram.getFunctionManager().getFunctions(true)) {
				long a = f.getEntryPoint().getOffset();
				if (a < encEnd && !code.contains(a)) fresh.add(a);
			}
			ReferenceManager rm = currentProgram.getReferenceManager();
			AddressSet enc = new AddressSet(toAddr(0), toAddr(encEnd - 1));
			for (Address dst : rm.getReferenceDestinationIterator(enc, true)) {
				long a = dst.getOffset();
				if (code.contains(a) || (a & 1) != 0) continue;
				for (Reference r : rm.getReferencesTo(dst)) {
					RefType t = r.getReferenceType();
					if (t.isFlow() || t.isData()) {
						// seulement les cibles référencées depuis du code sûr ou des données (pas depuis du code douteux)
						long from = r.getFromAddress().getOffset();
						if (from >= encEnd || code.contains(from) || t.isData()) {
							if (currentProgram.getListing().getInstructionAt(dst) != null) fresh.add(a);
						}
					}
				}
			}
			fresh.removeAll(entries);
			println(String.format("passe %d : %d octets de code chiffré, %d nouvelles entrées", pass, code.size(), fresh.size()));
			if (fresh.isEmpty()) break;
			entries.addAll(fresh);
		}

		// État final : mémoire fusionnée + analyse ; on exporte aussi l'image fusionnée.
		byte[] merged = merge(code);
		Files.write(outMerged.toPath(), merged);
		println("image fusionnée écrite : " + outMerged);
	}

	private final Set<Long> covBytes = new HashSet<>();
	private final List<Long> covEntries = new ArrayList<>();
	private AddressSet covRestrict;

	private void loadCoverage(File f) throws Exception {
		byte[] c = Files.readAllBytes(f.toPath());
		covRestrict = new AddressSet(toAddr(encEnd), toAddr(plain.length - 1));
		int n = Math.min(c.length, encEnd / 2);
		for (int w = 0; w < n; w++) {
			if (c[w] == 0) continue;
			long a = 2L * w;
			covBytes.add(a);
			covBytes.add(a + 1);
			if (w == 0 || c[w - 1] == 0) covEntries.add(a);
		}
		for (long a : covBytes) covRestrict.add(toAddr(a));
		println(String.format("couverture : %d octets, %d plages", covBytes.size(), covEntries.size()));
	}

	// Octets de données lus par adressage relatif au PC : ils passent par l'espace programme, donc déchiffrés.
	private final Set<Long> pcData = new HashSet<>();

	private int w16(long a) {
		return ((dec[(int) a] & 0xff) << 8) | (dec[(int) a + 1] & 0xff);
	}

	// Résout les idiomes Capcom que le flot seul ne suit pas :
	//  - jsr/jmp (d8,pc,Dn) précédé de move.w (d8,pc,Dn),Dn sur la même base : table d'offsets 16 bits
	//  - jsr/jmp (d8,pc,Dn) seul : table de bra.w (pas de 4)
	//  - lea (d16,pc),a6 : adresse de retour pour un appel sans pile (bra routine ; routine fait jmp (a6))
	private Set<Long> resolvePcTables() {
		Set<Long> out = new TreeSet<>();
		InstructionIterator it = currentProgram.getListing().getInstructions(
			new AddressSet(toAddr(0), toAddr(encEnd - 1)), true);
		while (it.hasNext()) {
			Instruction ins = it.next();
			long a = ins.getMinAddress().getOffset();
			int op = w16(a);
			if (op == 0x4dfa) {
				out.add(a + 2 + (short) w16(a + 2));
			}
			// lea abs.l,a6  |  lea abs.l,An ; jmp (An)  |  lea abs.l,a0 ; ... trap #0 (création de tâche)
			if ((op & 0xf1ff) == 0x41f9) {
				int an = (op >> 9) & 7;
				long t = ((long) w16(a + 2) << 16) | w16(a + 4);
				if (t < plain.length && (t & 1) == 0
					&& (an == 6 || w16(a + 6) == (0x4ed0 | an) || (an == 0 && spawnsTaskSoon(ins)))) out.add(t);
			}
			// movea.l (d8,pc,Dn),An ; jmp/jsr (An) : table de pointeurs longs lue en espace programme
			if ((op & 0xf1ff) == 0x207b) {
				int an = (op >> 9) & 7;
				int nxt = w16(a + 4);
				if (nxt == (0x4ed0 | an) || nxt == (0x4e90 | an)) {
					long base = a + 2 + (byte) w16(a + 2);
					for (int k = 0; k < 64; k++) {
						long slot = base + 4 * k;
						long t = ((long) w16(slot) << 16) | w16(slot + 2);
						if (t >= plain.length || (t & 1) != 0) {
							if (k == 0) continue;
							break;
						}
						for (int b = 0; b < 4; b++) pcData.add(slot + b);
						out.add(t);
					}
				}
			}
			if (op != 0x4ebb && op != 0x4efb) continue;
			long base = a + 2 + (byte) w16(a + 2);
			long prev = a - 4;
			boolean offsetTable = prev >= 0 && (w16(prev) & 0xf1ff) == 0x303b
				&& prev + 2 + (byte) w16(prev + 2) == base;
			if (offsetTable) {
				long end = encEnd;
				for (int k = 0; k < 512 && base + 2 * k < end; k++) {
					long slot = base + 2 * k;
					int v = (short) w16(slot);
					long t = base + v;
					if ((v & 1) != 0 || t < 0 || t >= encEnd || (t >= base && t <= slot)) break;
					pcData.add(slot);
					pcData.add(slot + 1);
					out.add(t);
					if (t > base) end = Math.min(end, t);
				}
			}
			else {
				for (int k = 0; k < 512; k++) {
					long slot = base + 4 * k;
					int w = w16(slot);
					if (w != 0x6000 && w != 0x4e75 && w != 0x4e71) break;
					out.add(slot);
				}
			}
		}
		return out;
	}

	// Vrai si l'une des 4 instructions suivantes crée une tâche : trap #0, ou appel d'une fonction
	// qui fait trap #0 dans ses premières instructions (ex. 0x9b6 : movem ; trap #0 ; movem ; rts).
	private boolean spawnsTaskSoon(Instruction ins) {
		Instruction cur = ins;
		for (int n = 0; n < 4 && cur != null; n++) {
			cur = cur.getNext();
			if (cur == null) break;
			long a = cur.getMinAddress().getOffset();
			if (a + 2 > dec.length) break;
			if (w16(a) == 0x4e40) return true;
			for (Address f : cur.getFlows()) {
				long t = f.getOffset();
				if (cur.getFlowType().isCall() && t + 8 <= dec.length) {
					for (int k = 0; k < 8; k += 2) if (w16(t + k) == 0x4e40) return true;
				}
			}
		}
		return false;
	}

	private byte[] merge(Set<Long> code) {
		byte[] m = plain.clone();
		for (long a : code) m[(int) a] = dec[(int) a];
		return m;
	}

	private Set<Long> instructionBytes(long end) {
		Set<Long> s = new HashSet<>();
		InstructionIterator it = currentProgram.getListing().getInstructions(
			new AddressSet(toAddr(0), toAddr(end - 1)), true);
		while (it.hasNext()) {
			Instruction ins = it.next();
			long a = ins.getMinAddress().getOffset();
			for (int i = 0; i < ins.getLength(); i++) s.add(a + i);
		}
		return s;
	}

	private void clearAll() throws Exception {
		AddressSet rom = new AddressSet(toAddr(0), toAddr(plain.length - 1));
		FunctionManager fm = currentProgram.getFunctionManager();
		List<Address> fns = new ArrayList<>();
		for (Function f : fm.getFunctions(rom, true)) fns.add(f.getEntryPoint());
		for (Address a : fns) fm.removeFunction(a);
		currentProgram.getListing().clearCodeUnits(toAddr(0), toAddr(plain.length - 1), false);
		currentProgram.getReferenceManager().removeAllReferencesFrom(toAddr(0), toAddr(plain.length - 1));
	}

	// Écrit uniquement les octets qui changent, par plages contiguës (Ghidra refuse d'écrire sous une instruction).
	private void writeRange(Memory mem, byte[] src, int start, int end) throws Exception {
		byte[] cur = new byte[end - start];
		mem.getBytes(toAddr(start), cur);
		int i = 0;
		while (i < cur.length) {
			if (cur[i] == src[start + i]) { i++; continue; }
			int j = i;
			while (j < cur.length && cur[j] != src[start + j]) j++;
			mem.setBytes(toAddr(start + i), Arrays.copyOfRange(src, start + i, start + j));
			i = j;
		}
	}

	private void setupMemoryMap(Memory mem) throws Exception {
		MemoryBlock rom = mem.getBlock(toAddr(0));
		rom.setName("ROM");
		rom.setWrite(false);
		rom.setExecute(true);
		uninit(mem, "OUTPUT", 0x400000, 0xc, true);
		uninit(mem, "QSOUND_SHARED", 0x618000, 0x2000, true);
		uninit(mem, "EXTRA_RAM", 0x660000, 0x4002, false);
		uninit(mem, "OBJRAM0", 0x700000, 0x2000, true);
		uninit(mem, "OBJRAM1", 0x708000, 0x8000, true);
		uninit(mem, "CPS_IO_MIRROR", 0x800100, 0x80, true);
		uninit(mem, "CPS_IO", 0x804000, 0x180, true);
		uninit(mem, "GFXRAM", 0x900000, 0x30000, false);
		uninit(mem, "WORKRAM", 0xff0000, 0x10000, false);
		// Adressage absolu court (xxx.w) : 0x8000..0xFFFF est étendu en 0xFFFF8000.. ; sur le bus 24 bits c'est 0xFF8000.
		MemoryBlock m = mem.createByteMappedBlock("WORKRAM_SHORT", toAddr(0xffff0000L), toAddr(0xff0000), 0x10000, false);
		m.setWrite(true);
	}

	private void uninit(Memory mem, String name, long start, long len, boolean vol) throws Exception {
		MemoryBlock b = mem.createUninitializedBlock(name, toAddr(start), len, false);
		b.setWrite(true);
		b.setVolatile(vol);
	}

	private void setupIoLabels() throws Exception {
		Object[][] io = {
			{0x400000L, "cps2_obj_out"},
			{0x618000L, "qsound_shared"},
			{0x700000L, "objram0"}, {0x708000L, "objram1"},
			{0x804000L, "io_in0"}, {0x804010L, "io_in1"}, {0x804020L, "io_in2_eeprom"},
			{0x804030L, "io_qsound_volume"}, {0x804040L, "io_eeprom_out"}, {0x8040a0L, "io_unk_8040a0"},
			{0x8040b0L, "io_dsw"}, {0x8040e0L, "io_objram_bank"},
			{0x804100L, "cps_a_regs"}, {0x804140L, "cps_b_regs"},
			{0x900000L, "gfxram"}, {0xff0000L, "workram"},
		};
		for (Object[] e : io) label((Long) e[0], (String) e[1]);
	}

	private void label(long addr, String name) throws Exception {
		createLabel(toAddr(addr), name, true, SourceType.USER_DEFINED);
	}

	private static long be32(byte[] b, int o) {
		return ((b[o] & 0xffL) << 24) | ((b[o + 1] & 0xffL) << 16) | ((b[o + 2] & 0xffL) << 8) | (b[o + 3] & 0xffL);
	}
}
