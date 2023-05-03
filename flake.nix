{
  inputs.nixpkgs.url = github:NixOS/nixpkgs/nixos-22.11;

  outputs = { self, nixpkgs }: let
    apkgs = import nixpkgs {
      system = "x86_64-linux";
    };
  in {
    devShells.x86_64-linux.default = with apkgs; mkShell {
      buildInputs = [
        pkg-config
        openssl
      ];
      LD_LIBRARY_PATH = lib.makeLibraryPath [
      ];
    };
  };
}
