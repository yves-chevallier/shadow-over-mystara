// Statistiques rapides : fonctions par zone et sauts indirects.
// @category CPS2
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.*;
import java.util.*;
public class Report extends GhidraScript {
	@Override protected void run() throws Exception {
		int[] fn = new int[5]; long[] ins = new long[5];
		for (Function f : currentProgram.getFunctionManager().getFunctions(true)) {
			long a = f.getEntryPoint().getOffset(); if (a < 0x400000) fn[(int)(a >> 20)]++;
		}
		Map<String,Integer> ind = new TreeMap<>();
		for (Instruction i : currentProgram.getListing().getInstructions(true)) {
			long a = i.getMinAddress().getOffset(); if (a >= 0x400000) continue;
			ins[(int)(a >> 20)] += i.getLength();
			String m = i.getMnemonicString();
			if ((m.startsWith("jmp") || m.startsWith("jsr")) && i.getReferencesFrom().length == 0) {
				String k = m + " " + i.getDefaultOperandRepresentation(0).replaceAll("0x[0-9a-f]+", "N");
				ind.merge(k, 1, Integer::sum);
			}
		}
		for (int r = 0; r < 4; r++) println(String.format("Mo %d : %d fonctions, %d octets d'instructions", r, fn[r], ins[r]));
		ind.entrySet().stream().sorted((x, y) -> y.getValue() - x.getValue()).limit(15)
			.forEach(e -> println("indirect " + e.getValue() + "  " + e.getKey()));
	}
}
