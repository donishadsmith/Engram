-- https://archives.glitchcity.info/forums/board-76/thread-1342/page-0.html
-- shinies!!!
local domain_name, offset = memory.address_to_domain(0xD230)
function emulator.on_frame()
    memory.write_domain(domain_name, offset, 7)
end

print(string.format("Domain: %s; Offset: %04X; Value: %d", domain_name, offset, memory.read_domain(domain_name, offset)))
