{ pkgs ? import <nixpkgs> {} }:
  pkgs.mkShell {
    packages = with pkgs; [
      rustup
      elf2uf2-rs
    ];

    shellHook = ''
      rustup target add thumbv6m-none-eabi
    '';
}
