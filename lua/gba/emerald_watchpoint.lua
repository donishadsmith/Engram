set_watchpoint(0x03001234, {pause = false, on = "read", access = "halfword"})

function on_watchpoint(hit)
    print(string.format("%s %s at %08X value=%04X from pc=%08X",
        hit.on, hit.access, hit.address, hit.value, hit.pc))
end
