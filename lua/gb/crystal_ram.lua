-- https://archives.glitchcity.info/forums/board-76/thread-1342/page-0.html
-- shinies!!!
-- and get the best mon
local domain_name_1, offset_1 = memory.address_to_domain(0xD230)
local domain_name_2, offset_2 = memory.address_to_domain(0xD204)

function emulator.on_frame()
    memory.write_domain(domain_name_1, offset_1, 0x7)
    memory.write_domain(domain_name_2, offset_2, 0xFB)
end

print(string.format("Domain: %s; Offset: %04X; Value: %d", domain_name_1, offset_1, memory.read_domain(domain_name_1, offset_1)))
print(string.format("Domain: %s; Offset: %04X; Value: %d", domain_name_2, offset_2, memory.read_domain(domain_name_2, offset_2)))
