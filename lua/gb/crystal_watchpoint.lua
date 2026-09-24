clear_all_breakpoints()

local log = io.open("encounters.txt", "a")

set_watchpoint(0xD206, {pause = false, on = "change"})

function on_watchpoint(hit)
    local line = string.format(
        "species=%3d written by rom+%05X (pc=%04X) into %s+%04X (address=%04X)",
        hit.value,
        hit.pc_offset,
        hit.pc,
        hit.domain,
        hit.offset,
        hit.address
    )

    print(line)
    log:write(line, "\n")
    log:flush()
end
