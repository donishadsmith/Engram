local functions = {}

function on_pc(address, fn)
	functions[address] = fn
	set_breakpoint(address)
end

function on_breakpoint(address)
	local fn = functions[address]
	if fn then
		fn(address)
	end
end

on_pc(0x080E518C, function(pc)
	print("r0", read_cpu_register("r0"))
end)
