// Exporte le pseudo-C décompilé de fonctions : <sortie.c> <adresse_hex> [<adresse_hex> ...]
// (crée la fonction si elle n'existe pas). Sert à accélérer la réécriture en Rust.
// @category CPS2
import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import java.io.PrintWriter;
public class Decompile extends GhidraScript {
	@Override protected void run() throws Exception {
		String[] args = getScriptArgs();
		DecompInterface d = new DecompInterface();
		d.openProgram(currentProgram);
		try (PrintWriter w = new PrintWriter(args[0])) {
			for (int i = 1; i < args.length; i++) {
				Address a = toAddr(Long.parseLong(args[i], 16));
				Function f = getFunctionAt(a);
				if (f == null) f = createFunction(a, null);
				if (f == null) { w.println("// pas de fonction en " + args[i]); continue; }
				DecompileResults r = d.decompileFunction(f, 60, monitor);
				w.println("// ===== " + f.getName() + " @ " + args[i]);
				w.println(r.decompileCompleted() ? r.getDecompiledFunction().getC() : "// échec : " + r.getErrorMessage());
			}
		}
	}
}
