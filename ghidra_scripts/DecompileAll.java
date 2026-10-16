// Décompile toutes les fonctions dans <dossier>/XXXXXX.c et écrit <dossier>/index.tsv :
// adresse, nom, taille (octets), appelants, appelés. Sert de base de travail aux réécritures.
// @category CPS2
import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;
import java.io.File;
import java.io.PrintWriter;
import java.util.*;
public class DecompileAll extends GhidraScript {
	@Override protected void run() throws Exception {
		File dir = new File(getScriptArgs()[0]);
		dir.mkdirs();
		DecompInterface d = new DecompInterface();
		d.openProgram(currentProgram);
		try (PrintWriter idx = new PrintWriter(new File(dir, "index.tsv"))) {
			idx.println("adresse\tnom\ttaille\tappelants\tappeles");
			for (Function f : currentProgram.getFunctionManager().getFunctions(true)) {
				long a = f.getEntryPoint().getOffset();
				if (a >= 0x400000) continue;
				StringBuilder callers = new StringBuilder(), callees = new StringBuilder();
				for (Function c : f.getCallingFunctions(monitor)) callers.append(String.format("%06x ", c.getEntryPoint().getOffset()));
				for (Function c : f.getCalledFunctions(monitor)) callees.append(String.format("%06x ", c.getEntryPoint().getOffset()));
				idx.printf("%06x\t%s\t%d\t%s\t%s%n", a, f.getName(), f.getBody().getNumAddresses(), callers.toString().trim(), callees.toString().trim());
				DecompileResults r = d.decompileFunction(f, 30, monitor);
				try (PrintWriter w = new PrintWriter(new File(dir, String.format("%06x.c", a)))) {
					w.println(r.decompileCompleted() ? r.getDecompiledFunction().getC() : "// échec : " + r.getErrorMessage());
				}
			}
		}
	}
}
